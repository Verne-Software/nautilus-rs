//! The signed-token verifier, against tokens signed here with locally generated P-256 keys.
#![cfg(feature = "jwt")]

mod common;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use aws_lc_rs::{
    rand::SystemRandom,
    signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_FIXED_SIGNING},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use common::{JSON, KEY, ORG, USER};
use mockito::{Server, ServerGuard};
use nautilus_rs::{Error, Gate, GateTokenVerifier, TokenErrorReason};
use serde_json::{json, Value};

const TENANT: &str = "550e8400-e29b-41d4-a716-446655440000";

struct Pair {
    key: EcdsaKeyPair,
    kid: String,
}

impl Pair {
    fn new(kid: &str) -> Self {
        let rng = SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).unwrap();
        let key =
            EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8.as_ref()).unwrap();
        Self {
            key,
            kid: kid.into(),
        }
    }

    /// The public half as a JWK: `x` and `y` are the 32-byte halves of `0x04 ‖ x ‖ y`.
    fn jwk(&self) -> Value {
        let point = self.key.public_key().as_ref();
        json!({
            "kty": "EC", "crv": "P-256", "alg": "ES256", "use": "sig", "kid": self.kid,
            "x": B64.encode(&point[1..33]),
            "y": B64.encode(&point[33..65]),
        })
    }

    fn sign(&self, claims: &Value) -> String {
        self.sign_with(
            &json!({"alg": "ES256", "typ": "JWT", "kid": self.kid}),
            claims,
        )
    }

    fn sign_with(&self, header: &Value, claims: &Value) -> String {
        let input = format!(
            "{}.{}",
            B64.encode(header.to_string()),
            B64.encode(claims.to_string())
        );
        let signature = self
            .key
            .sign(&SystemRandom::new(), input.as_bytes())
            .unwrap();
        format!("{input}.{}", B64.encode(signature.as_ref()))
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn issuer(server: &ServerGuard) -> String {
    format!("{}/auth/{TENANT}", server.url())
}

fn claims(server: &ServerGuard) -> Value {
    claims_with(server, json!({}))
}

fn claims_with(server: &ServerGuard, overrides: Value) -> Value {
    let now = now();
    let mut claims = json!({
        "iss": issuer(server),
        "sub": USER,
        "sid": "8e4f0b2a-3c1d-4e5f-9a6b-7c8d9e0f1a2b",
        "tenant_id": TENANT,
        "schema_id": "user",
        "email": "ada@example.com",
        "role": "authenticated",
        "orgs": [{"id": ORG, "role": "admin", "permissions": ["invoices:write"]}],
        "iat": now, "nbf": now, "exp": now + 900,
        "jti": "j1",
    });
    for (k, v) in overrides.as_object().unwrap() {
        claims[k] = v.clone();
    }
    claims
}

const JWKS_PATH: &str = "/auth/550e8400-e29b-41d4-a716-446655440000/.well-known/jwks.json";

fn jwks_body(keys: &[&Pair]) -> String {
    json!({ "keys": keys.iter().map(|k| k.jwk()).collect::<Vec<_>>() }).to_string()
}

async fn serve(server: &mut ServerGuard, keys: &[&Pair]) -> mockito::Mock {
    server
        .mock("GET", JWKS_PATH)
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_header("cache-control", "public, max-age=300")
        .with_body(jwks_body(keys))
        .create_async()
        .await
}

fn verifier(server: &ServerGuard) -> GateTokenVerifier {
    GateTokenVerifier::builder()
        .tenant_id(TENANT)
        .base_url(server.url())
        .build()
        .unwrap()
}

fn reason(err: Error) -> TokenErrorReason {
    match err {
        Error::Token(e) => e.reason,
        other => panic!("expected Error::Token, got {other:?}"),
    }
}

#[tokio::test]
async fn accepts_a_token_signed_by_the_tenant_key_and_caches_the_key_set() {
    let mut server = Server::new_async().await;
    let pair = Pair::new("k1");
    let jwks = serve(&mut server, &[&pair]).await.expect(1);

    let verifier = verifier(&server);
    assert_eq!(verifier.issuer(), issuer(&server));

    let claims = verifier.verify(&pair.sign(&claims(&server))).await.unwrap();
    assert_eq!(claims.sub, USER);
    assert_eq!(claims.tenant_id, TENANT);
    assert_eq!(claims.role, "authenticated");
    assert_eq!(claims.email.as_deref(), Some("ada@example.com"));
    assert_eq!(claims.orgs.len(), 1);
    assert_eq!(
        (claims.orgs[0].id.as_str(), claims.orgs[0].role.as_str()),
        (ORG, "admin")
    );
    assert_eq!(claims.orgs_truncated, None);

    // A second token does not refetch the key set (expect(1) above).
    verifier
        .verify(&pair.sign(&claims_with(&server, json!({"jti": "j2"}))))
        .await
        .unwrap();
    jwks.assert_async().await;
}

#[tokio::test]
async fn refuses_the_ways_a_token_can_be_wrong() {
    let mut server = Server::new_async().await;
    let pair = Pair::new("k1");
    let other = Pair::new("k1"); // a different key under the same kid
    serve(&mut server, &[&pair]).await;
    let verifier = verifier(&server);
    let now = now();

    let alg_none = format!(
        "{}.{}.",
        B64.encode(json!({"alg": "none", "kid": "k1"}).to_string()),
        B64.encode(claims(&server).to_string())
    );
    let hs256 = pair.sign_with(&json!({"alg": "HS256", "kid": "k1"}), &claims(&server));
    let no_kid = pair.sign_with(&json!({"alg": "ES256"}), &claims(&server));

    let cases: Vec<(&str, String, TokenErrorReason)> = vec![
        ("garbage", "not-a-jwt".into(), TokenErrorReason::Malformed),
        ("four parts", "a.b.c.d".into(), TokenErrorReason::Malformed),
        (
            "non-base64 header",
            "!!!.e30.e30".into(),
            TokenErrorReason::Malformed,
        ),
        ("alg none", alg_none, TokenErrorReason::UnsupportedAlg),
        ("alg HS256", hs256, TokenErrorReason::UnsupportedAlg),
        ("no kid", no_kid, TokenErrorReason::Malformed),
        (
            "another key under our kid",
            other.sign(&claims(&server)),
            TokenErrorReason::BadSignature,
        ),
        (
            "another tenant's issuer",
            pair.sign(&claims_with(
                &server,
                json!({"iss": "https://api.vernesoft.com/auth/other"}),
            )),
            TokenErrorReason::WrongIssuer,
        ),
        (
            "a missing issuer",
            pair.sign(&claims_with(&server, json!({"iss": null}))),
            TokenErrorReason::WrongIssuer,
        ),
        (
            "an expired token",
            pair.sign(&claims_with(&server, json!({"exp": now - 60}))),
            TokenErrorReason::Expired,
        ),
        (
            "nbf in the future",
            pair.sign(&claims_with(&server, json!({"nbf": now + 600}))),
            TokenErrorReason::NotYetValid,
        ),
        (
            "iat in the future",
            pair.sign(&claims_with(&server, json!({"iat": now + 600}))),
            TokenErrorReason::NotYetValid,
        ),
        (
            "no exp",
            pair.sign(&claims_with(&server, json!({"exp": null}))),
            TokenErrorReason::MissingClaim,
        ),
        (
            "an empty sub",
            pair.sign(&claims_with(&server, json!({"sub": ""}))),
            TokenErrorReason::MissingClaim,
        ),
    ];

    for (label, token, expected) in cases {
        let err = verifier.verify(&token).await.unwrap_err();
        assert_eq!(reason(err), expected, "{label}");
    }
}

/// A valid signature is not enough: the payload is bound to the signature.
#[tokio::test]
async fn refuses_a_token_whose_payload_was_swapped() {
    let mut server = Server::new_async().await;
    let pair = Pair::new("k1");
    serve(&mut server, &[&pair]).await;
    let verifier = verifier(&server);

    let genuine = pair.sign(&claims(&server));
    let forged_payload =
        B64.encode(claims_with(&server, json!({"sub": "someone-else"})).to_string());
    let parts: Vec<&str> = genuine.split('.').collect();
    let forged = format!("{}.{}.{}", parts[0], forged_payload, parts[2]);

    let err = verifier.verify(&forged).await.unwrap_err();
    assert_eq!(reason(err), TokenErrorReason::BadSignature);
}

#[tokio::test]
async fn a_signature_of_the_wrong_length_is_a_bad_signature() {
    let mut server = Server::new_async().await;
    let pair = Pair::new("k1");
    serve(&mut server, &[&pair]).await;
    let verifier = verifier(&server);

    let genuine = pair.sign(&claims(&server));
    let truncated = format!(
        "{}.{}",
        &genuine[..genuine.rfind('.').unwrap()],
        B64.encode([1u8; 63])
    );
    assert_eq!(
        reason(verifier.verify(&truncated).await.unwrap_err()),
        TokenErrorReason::BadSignature
    );
}

#[tokio::test]
async fn tolerates_a_little_clock_skew_and_not_more() {
    let mut server = Server::new_async().await;
    let pair = Pair::new("k1");
    serve(&mut server, &[&pair]).await;
    let verifier = verifier(&server);
    let now = now();

    // Expired 5 s ago: inside the default 10 s leeway.
    verifier
        .verify(&pair.sign(&claims_with(&server, json!({"exp": now - 5}))))
        .await
        .unwrap();

    let strict = GateTokenVerifier::builder()
        .tenant_id(TENANT)
        .base_url(server.url())
        .leeway_secs(0)
        .build()
        .unwrap();
    let err = strict
        .verify(&pair.sign(&claims_with(&server, json!({"exp": now - 5}))))
        .await
        .unwrap_err();
    assert_eq!(reason(err), TokenErrorReason::Expired);
}

#[tokio::test]
async fn an_unknown_kid_refetches_the_key_set_once_a_rotation_is_followed() {
    let mut server = Server::new_async().await;
    let current = Pair::new("k1");
    let next = Pair::new("k2");
    let stranger = Pair::new("k3");

    // First fetch: only k1. Later fetches: k1 and k2.
    let first = server
        .mock("GET", JWKS_PATH)
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(jwks_body(&[&current]))
        .expect(1)
        .create_async()
        .await;

    // A short refetch interval stands in for the 30 seconds.
    let verifier = GateTokenVerifier::builder()
        .tenant_id(TENANT)
        .base_url(server.url())
        .min_refetch_interval(Duration::from_millis(300))
        .build()
        .unwrap();

    verifier
        .verify(&current.sign(&claims(&server)))
        .await
        .unwrap();
    first.assert_async().await;
    first.remove_async().await;

    let rotated = server
        .mock("GET", JWKS_PATH)
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(jwks_body(&[&current, &next]))
        .expect(1)
        .create_async()
        .await;

    // k2 is unknown, but the key set is younger than the refetch interval: not refetched.
    let err = verifier
        .verify(&next.sign(&claims(&server)))
        .await
        .unwrap_err();
    assert_eq!(reason(err), TokenErrorReason::UnknownKid);

    // Past the interval, an unknown kid asks again, finds k2, and accepts.
    tokio::time::sleep(Duration::from_millis(350)).await;
    verifier.verify(&next.sign(&claims(&server))).await.unwrap();
    rotated.assert_async().await;

    // A kid nobody publishes: refused, and not refetched again within the interval.
    let err = verifier
        .verify(&stranger.sign(&claims(&server)))
        .await
        .unwrap_err();
    assert_eq!(reason(err), TokenErrorReason::UnknownKid);
    rotated.assert_async().await; // still exactly one rotated fetch
}

#[tokio::test]
async fn an_unreachable_key_set_is_jwks_unavailable() {
    let mut server = Server::new_async().await;
    let pair = Pair::new("k1");
    server
        .mock("GET", JWKS_PATH)
        .with_status(503)
        .create_async()
        .await;

    let err = verifier(&server)
        .verify(&pair.sign(&claims(&server)))
        .await
        .unwrap_err();
    assert_eq!(reason(err), TokenErrorReason::JwksUnavailable);

    // And a body that is not a key set.
    let mut bad = Server::new_async().await;
    bad.mock("GET", JWKS_PATH)
        .with_status(200)
        .with_body(r#"{"nope":1}"#)
        .create_async()
        .await;
    let err = verifier(&bad)
        .verify(&pair.sign(&claims(&bad)))
        .await
        .unwrap_err();
    assert_eq!(reason(err), TokenErrorReason::JwksUnavailable);
}

#[tokio::test]
async fn keys_that_are_not_p256_signing_keys_are_skipped() {
    let mut server = Server::new_async().await;
    let pair = Pair::new("k1");
    let mut rsa = json!({"kty": "RSA", "kid": "r1", "n": "AQAB", "e": "AQAB"});
    let mut p384 = pair.jwk();
    p384["crv"] = json!("P-384");
    p384["kid"] = json!("p384");
    let mut enc = pair.jwk();
    enc["use"] = json!("enc");
    enc["kid"] = json!("enc");
    rsa["use"] = json!("sig");
    server
        .mock("GET", JWKS_PATH)
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(json!({"keys": [rsa, p384, enc, pair.jwk()]}).to_string())
        .create_async()
        .await;

    let verifier = verifier(&server);
    verifier.verify(&pair.sign(&claims(&server))).await.unwrap();
    // The encryption-use key is not a signing key, even though it is a valid P-256 point.
    let as_enc = Pair {
        kid: "enc".into(),
        ..Pair::new("enc")
    };
    assert_eq!(
        reason(
            verifier
                .verify(&as_enc.sign(&claims(&server)))
                .await
                .unwrap_err()
        ),
        TokenErrorReason::UnknownKid
    );
}

#[tokio::test]
async fn gate_tokens_verify_signed_builds_the_issuer_from_the_clients_base_url() {
    let mut server = Server::new_async().await;
    let pair = Pair::new("k1");
    let jwks = serve(&mut server, &[&pair]).await.expect(1);

    let gate = Gate::builder()
        .api_key(KEY)
        // a trailing slash must not end up in the issuer
        .base_url(format!("{}/", server.url()))
        .build()
        .unwrap();

    let token = pair.sign(&claims(&server));
    let verified = gate.tokens().verify_signed(&token, TENANT).await.unwrap();
    assert_eq!(verified.iss, issuer(&server));

    // The verifier is kept between calls: the key set is fetched once.
    gate.tokens().verify_signed(&token, TENANT).await.unwrap();
    jwks.assert_async().await;

    // Another tenant id is another issuer: the same token is refused, whatever its signature.
    let mut other = Server::new_async().await;
    other
        .mock("GET", "/auth/another-tenant/.well-known/jwks.json")
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(jwks_body(&[&pair]))
        .create_async()
        .await;
    let foreign = GateTokenVerifier::builder()
        .tenant_id("another-tenant")
        .base_url(other.url())
        .build()
        .unwrap();
    assert_eq!(
        reason(foreign.verify(&token).await.unwrap_err()),
        TokenErrorReason::WrongIssuer
    );
}

#[tokio::test]
async fn an_empty_tenant_id_is_a_configuration_error() {
    assert!(matches!(GateTokenVerifier::new(""), Err(Error::Config(_))));
    assert!(matches!(
        GateTokenVerifier::builder().build(),
        Err(Error::Config(_))
    ));
    let v = GateTokenVerifier::new(TENANT).unwrap();
    assert_eq!(
        v.issuer(),
        format!("https://api.vernesoft.com/auth/{TENANT}")
    );
    assert_eq!(
        v.jwks_url(),
        format!("https://api.vernesoft.com/auth/{TENANT}/.well-known/jwks.json")
    );
}

/// The verify future can be spawned onto a multi-threaded runtime.
#[tokio::test]
async fn verify_is_send() {
    let mut server = Server::new_async().await;
    let pair = Pair::new("k1");
    serve(&mut server, &[&pair]).await;
    let verifier = std::sync::Arc::new(verifier(&server));
    let token = pair.sign(&claims(&server));

    let v = std::sync::Arc::clone(&verifier);
    let handle = tokio::spawn(async move { v.verify(&token).await });
    assert_eq!(handle.await.unwrap().unwrap().sub, USER);
}
