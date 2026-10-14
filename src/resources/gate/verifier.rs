use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use aws_lc_rs::signature::{UnparsedPublicKey, ECDSA_P256_SHA256_FIXED};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::{Map, Value};

use super::types::SignedTokenClaims;
use crate::{
    error::{Error, TokenError, TokenErrorReason},
    http::{normalize_base_url, API_URL},
};

/// Cache lifetime when the key set answers without `Cache-Control: max-age`.
const DEFAULT_JWKS_MAX_AGE: Duration = Duration::from_secs(300);
/// An unknown `kid` refetches the key set at most this often.
const DEFAULT_MIN_REFETCH_INTERVAL: Duration = Duration::from_secs(30);
const DEFAULT_LEEWAY_SECS: u64 = 10;
const DEFAULT_TIMEOUT_SECS: u64 = 10;

/// One verifier per tenant id, shared by a [`Gate`](super::Gate)'s `tokens()` clients.
pub(super) type VerifierCache = Arc<Mutex<HashMap<String, Arc<GateTokenVerifier>>>>;

/// The tenant's public keys: `kid` → the key as an uncompressed SEC1 point
/// (`0x04 ‖ x ‖ y`).
#[derive(Default)]
struct KeySet {
    keys: HashMap<String, Vec<u8>>,
    fetched_at: Option<Instant>,
    max_age: Duration,
}

/// Verifies Gate signed tokens (ES256 JWTs) offline, against your tenant's published
/// public keys. No API key is involved: anything that receives a token can verify it.
///
/// Needs the `jwt` feature.
///
/// What it checks: the algorithm is ES256 (anything else, `none` included, is refused),
/// the key is the one the token's `kid` names in your tenant's key set, the signature,
/// `iss` is **your** issuer (a token of another Gate tenant is refused even though its
/// signature is valid), `exp` and `nbf`, and that `sub` is present.
///
/// What it cannot check: whether the session behind the token is still alive. A signed
/// token lives 15 minutes and **cannot be revoked**; for anything that must follow a
/// revocation or a role change at once, ask
/// [`SessionsClient::check`](super::SessionsClient::check) or introspection.
///
/// The key set is cached for its `Cache-Control: max-age` (five minutes without one) and
/// refetched once when a token names a key it has not seen — which is how a key rotation
/// is followed — but not more often than every 30 seconds. Keep one verifier per tenant
/// and reuse it; it is cheap to share (`Arc`) between tasks.
///
/// # Example
///
/// ```no_run
/// use nautilus_rs::GateTokenVerifier;
///
/// # async fn run(token: &str) -> Result<(), nautilus_rs::Error> {
/// let verifier = GateTokenVerifier::new("550e8400-e29b-41d4-a716-446655440000")?;
/// let claims = verifier.verify(token).await?;
/// println!("user {}", claims.sub);
/// for org in &claims.orgs {
///     println!("{} in {}", org.role, org.id);
/// }
/// # Ok(())
/// # }
/// ```
pub struct GateTokenVerifier {
    issuer: String,
    jwks_url: String,
    leeway_secs: u64,
    min_refetch_interval: Duration,
    client: reqwest::Client,
    keys: Mutex<KeySet>,
}

impl std::fmt::Debug for GateTokenVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GateTokenVerifier")
            .field("issuer", &self.issuer)
            .finish_non_exhaustive()
    }
}

/// Builder for a [`GateTokenVerifier`].
///
/// # Example
///
/// ```no_run
/// use nautilus_rs::GateTokenVerifier;
///
/// let verifier = GateTokenVerifier::builder()
///     .tenant_id("550e8400-e29b-41d4-a716-446655440000")
///     .base_url("http://api.soft.com")
///     .leeway_secs(30)
///     .build()
///     .expect("invalid configuration");
/// ```
#[derive(Default)]
pub struct GateTokenVerifierBuilder {
    tenant_id: Option<String>,
    base_url: Option<String>,
    leeway_secs: Option<u64>,
    min_refetch_interval: Option<Duration>,
    timeout_secs: Option<u64>,
}

impl GateTokenVerifierBuilder {
    /// Your tenant id (**required**). The issuer is `<base URL>/auth/<tenant id>`.
    pub fn tenant_id(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }

    /// Override the API base URL (default: `https://api.vernesoft.com`) — the same one
    /// the API client uses.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Allowed clock skew in seconds for `exp`, `nbf` and `iat` (default: `10`).
    pub fn leeway_secs(mut self, secs: u64) -> Self {
        self.leeway_secs = Some(secs);
        self
    }

    /// The shortest time between two refetches of the key set that an unknown `kid`
    /// may cause (default: 30 seconds).
    pub fn min_refetch_interval(mut self, interval: Duration) -> Self {
        self.min_refetch_interval = Some(interval);
        self
    }

    /// Timeout of a key-set request in seconds (default: `10`).
    pub fn timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

    /// Consume the builder and return a [`GateTokenVerifier`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] if the tenant id is missing or empty, or the HTTP client
    /// cannot be initialised.
    pub fn build(self) -> Result<GateTokenVerifier, Error> {
        let tenant_id = self
            .tenant_id
            .filter(|id| !id.is_empty())
            .ok_or_else(|| Error::Config("GateTokenVerifier needs your tenant id".into()))?;
        let base = normalize_base_url(Some(self.base_url.unwrap_or_else(|| API_URL.into())));
        let issuer = format!("{base}/auth/{tenant_id}");
        let jwks_url = format!("{issuer}/.well-known/jwks.json");

        let client = reqwest::ClientBuilder::new()
            .timeout(Duration::from_secs(
                self.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS),
            ))
            .build()
            .map_err(Error::Http)?;

        Ok(GateTokenVerifier {
            issuer,
            jwks_url,
            leeway_secs: self.leeway_secs.unwrap_or(DEFAULT_LEEWAY_SECS),
            min_refetch_interval: self
                .min_refetch_interval
                .unwrap_or(DEFAULT_MIN_REFETCH_INTERVAL),
            client,
            keys: Mutex::new(KeySet::default()),
        })
    }
}

impl GateTokenVerifier {
    /// A verifier for `tenant_id` against the default base URL, `https://api.vernesoft.com`.
    /// Use [`builder`](Self::builder) for a local stack or a different leeway.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] if `tenant_id` is empty.
    pub fn new(tenant_id: impl Into<String>) -> Result<Self, Error> {
        Self::builder().tenant_id(tenant_id).build()
    }

    /// Return a [`GateTokenVerifierBuilder`] for fine-grained configuration.
    pub fn builder() -> GateTokenVerifierBuilder {
        GateTokenVerifierBuilder::default()
    }

    /// The issuer every accepted token carries: `<base URL>/auth/<tenant id>`.
    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    /// Where the tenant's public keys are fetched from.
    pub fn jwks_url(&self) -> &str {
        &self.jwks_url
    }

    /// Verify `token` and return its claims.
    ///
    /// Returns [`Error::Token`] — with a [`reason`](TokenError::reason) — when the token
    /// must not be trusted. Nothing else is returned: a failure to reach the key set is
    /// [`JwksUnavailable`](TokenErrorReason::JwksUnavailable).
    pub async fn verify(&self, token: &str) -> Result<SignedTokenClaims, Error> {
        self.verify_token(token).await.map_err(Error::Token)
    }

    async fn verify_token(&self, token: &str) -> Result<SignedTokenClaims, TokenError> {
        use TokenErrorReason as R;

        let parts: Vec<&str> = token.split('.').collect();
        let [raw_header, raw_payload, raw_signature] = parts[..] else {
            return Err(TokenError::new(R::Malformed, "Not a compact JWT."));
        };
        let header = decode_json(raw_header, "header")?;
        let payload = decode_json(raw_payload, "payload")?;

        match header.get("alg").and_then(Value::as_str) {
            Some("ES256") => {}
            other => {
                return Err(TokenError::new(
                    R::UnsupportedAlg,
                    format!(
                        "Token algorithm {} is not accepted; Gate signs with ES256.",
                        other.map_or_else(|| "(none given)".to_owned(), |alg| format!("{alg:?}"))
                    ),
                ))
            }
        }
        let kid = match header.get("kid").and_then(Value::as_str) {
            Some(kid) if !kid.is_empty() => kid,
            _ => return Err(TokenError::new(R::Malformed, "Token header has no `kid`.")),
        };

        let key = self.key_for(kid).await?;
        let signature = URL_SAFE_NO_PAD
            .decode(raw_signature)
            .ok()
            .filter(|s| s.len() == 64)
            .ok_or_else(|| {
                TokenError::new(R::BadSignature, "Token signature is not a P-256 signature.")
            })?;
        let signing_input = format!("{raw_header}.{raw_payload}");
        UnparsedPublicKey::new(&ECDSA_P256_SHA256_FIXED, &key)
            .verify(signing_input.as_bytes(), &signature)
            .map_err(|_| TokenError::new(R::BadSignature, "Token signature does not verify."))?;

        if payload.get("iss").and_then(Value::as_str) != Some(self.issuer.as_str()) {
            return Err(TokenError::new(
                R::WrongIssuer,
                format!(
                    "Token issuer {} is not {}.",
                    payload.get("iss").unwrap_or(&Value::Null),
                    self.issuer
                ),
            ));
        }

        let now = unix_now();
        let leeway = self.leeway_secs as f64;
        let Some(exp) = payload.get("exp").and_then(Value::as_f64) else {
            return Err(TokenError::new(R::MissingClaim, "Token has no `exp`."));
        };
        if now > exp + leeway {
            return Err(TokenError::new(R::Expired, "Token has expired."));
        }
        for claim in ["nbf", "iat"] {
            if payload
                .get(claim)
                .and_then(Value::as_f64)
                .is_some_and(|at| at > now + leeway)
            {
                return Err(TokenError::new(
                    R::NotYetValid,
                    format!("Token is not valid yet (`{claim}` is in the future)."),
                ));
            }
        }
        if !matches!(payload.get("sub"), Some(Value::String(sub)) if !sub.is_empty()) {
            return Err(TokenError::new(R::MissingClaim, "Token has no `sub`."));
        }

        serde_json::from_value(Value::Object(payload)).map_err(|e| {
            TokenError::new(
                R::Malformed,
                format!("Token claims are not as expected: {e}"),
            )
        })
    }

    fn lock(&self) -> MutexGuard<'_, KeySet> {
        // A poisoned lock only means another verification panicked; the key set is a map
        // that is replaced whole, so it is never half-written.
        self.keys.lock().unwrap_or_else(|e| e.into_inner())
    }

    async fn key_for(&self, kid: &str) -> Result<Vec<u8>, TokenError> {
        let (stale, empty) = {
            let set = self.lock();
            (
                set.fetched_at.map_or(true, |at| at.elapsed() > set.max_age),
                set.keys.is_empty(),
            )
        };
        if stale {
            self.refresh(empty).await?;
        }

        if let Some(key) = self.lock().keys.get(kid) {
            return Ok(key.clone());
        }
        let may_refetch = self
            .lock()
            .fetched_at
            .map_or(true, |at| at.elapsed() >= self.min_refetch_interval);
        if may_refetch {
            // A kid we have not seen: a new key may have started signing. Ask once.
            self.refresh(true).await?;
            if let Some(key) = self.lock().keys.get(kid) {
                return Ok(key.clone());
            }
        }
        Err(TokenError::new(
            TokenErrorReason::UnknownKid,
            format!("No key {kid} in {}.", self.jwks_url),
        ))
    }

    /// Fetch the key set. With `required` false a failure keeps the keys already cached
    /// (a stale key set is still the right one unless a token says otherwise).
    async fn refresh(&self, required: bool) -> Result<(), TokenError> {
        match self.load().await {
            Err(e) if required => Err(e),
            _ => Ok(()),
        }
    }

    async fn load(&self) -> Result<(), TokenError> {
        use TokenErrorReason::JwksUnavailable as Unavailable;

        let resp = self
            .client
            .get(&self.jwks_url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| {
                TokenError::new(
                    Unavailable,
                    format!("Could not fetch {}: {e}", self.jwks_url),
                )
            })?;
        if !resp.status().is_success() {
            return Err(TokenError::new(
                Unavailable,
                format!(
                    "{} answered HTTP {}.",
                    self.jwks_url,
                    resp.status().as_u16()
                ),
            ));
        }
        let max_age = resp
            .headers()
            .get("Cache-Control")
            .and_then(|v| v.to_str().ok())
            .and_then(max_age)
            .unwrap_or(DEFAULT_JWKS_MAX_AGE);
        let body: Value = resp.json().await.map_err(|_| {
            TokenError::new(
                Unavailable,
                format!("{} did not answer JSON.", self.jwks_url),
            )
        })?;
        let Some(jwks) = body.get("keys").and_then(Value::as_array) else {
            return Err(TokenError::new(
                Unavailable,
                format!("{} has no `keys`.", self.jwks_url),
            ));
        };

        // A key we cannot import is a key we cannot verify with; skip it.
        let keys = jwks.iter().filter_map(import_jwk).collect();
        *self.lock() = KeySet {
            keys,
            fetched_at: Some(Instant::now()),
            max_age,
        };
        Ok(())
    }
}

/// `(kid, 0x04 ‖ x ‖ y)` of a P-256 signing key from a JWK; `None` for anything else.
fn import_jwk(jwk: &Value) -> Option<(String, Vec<u8>)> {
    let text = |member: &str| jwk.get(member).and_then(Value::as_str);
    if text("kty")? != "EC" || text("crv")? != "P-256" {
        return None;
    }
    if text("use").is_some_and(|u| u != "sig") {
        return None;
    }
    let kid = text("kid")?.to_owned();
    let coordinate = |member: &str| {
        URL_SAFE_NO_PAD
            .decode(text(member)?)
            .ok()
            .filter(|c| c.len() == 32)
    };
    let mut point = Vec::with_capacity(65);
    point.push(0x04);
    point.extend(coordinate("x")?);
    point.extend(coordinate("y")?);
    Some((kid, point))
}

/// The `max-age` of a `Cache-Control` header value.
fn max_age(cache_control: &str) -> Option<Duration> {
    cache_control.split(',').find_map(|directive| {
        let (name, value) = directive.trim().split_once('=')?;
        name.trim()
            .eq_ignore_ascii_case("max-age")
            .then(|| value.trim().parse().ok().map(Duration::from_secs))?
    })
}

fn decode_json(segment: &str, what: &str) -> Result<Map<String, Value>, TokenError> {
    URL_SAFE_NO_PAD
        .decode(segment)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|value| match value {
            Value::Object(object) => Some(object),
            _ => None,
        })
        .ok_or_else(|| {
            TokenError::new(
                TokenErrorReason::Malformed,
                format!("Token {what} is not base64url JSON."),
            )
        })
}

fn unix_now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}
