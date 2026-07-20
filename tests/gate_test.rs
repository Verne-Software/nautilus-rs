use mockito::Server;
use nautilus::{
    AuthorizeParams, CreateIdentityParams, CreateTokenParams, Gate, IdentityTraitsInput,
    JsonPatchOp, SecuritySettings,
};

fn identity_json(id: &str) -> String {
    format!(
        r#"{{"id":"{id}","schema_id":"user","state":"active","traits":{{"email":"user@example.com","tenant_id":"ten_001"}}}}"#
    )
}

#[tokio::test]
async fn test_create_identity() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("POST", "/v1/gate/identities")
        .with_status(201)
        .with_header("content-type", "application/json")
        .with_body(identity_json("idn_001"))
        .create_async()
        .await;

    let gate = Gate::builder()
        .api_key("vrn_gate_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    let identity = gate
        .identities()
        .create(CreateIdentityParams {
            schema_id: "user".into(),
            traits: IdentityTraitsInput {
                email: "user@example.com".into(),
                custom_data: None,
            },
            credentials: None,
            state: Some("active".into()),
        })
        .await
        .unwrap();

    assert_eq!(identity.id, "idn_001");
    assert_eq!(identity.schema_id, "user");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_get_identity() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/v1/gate/identities/idn_001")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(identity_json("idn_001"))
        .create_async()
        .await;

    let gate = Gate::builder()
        .api_key("vrn_gate_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    let identity = gate.identities().get("idn_001").await.unwrap();

    assert_eq!(identity.id, "idn_001");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_patch_identity() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("PATCH", "/v1/gate/identities/idn_001")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(identity_json("idn_001"))
        .create_async()
        .await;

    let gate = Gate::builder()
        .api_key("vrn_gate_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    let identity = gate
        .identities()
        .patch(
            "idn_001",
            vec![JsonPatchOp {
                op: "replace".into(),
                path: "/traits/custom_data/role".into(),
                value: Some(serde_json::json!("admin")),
                from: None,
            }],
        )
        .await
        .unwrap();

    assert_eq!(identity.id, "idn_001");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_delete_identity() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("DELETE", "/v1/gate/identities/idn_001")
        .with_status(204)
        .create_async()
        .await;

    let gate = Gate::builder()
        .api_key("vrn_gate_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    gate.identities().delete("idn_001").await.unwrap();
    mock.assert_async().await;
}

#[tokio::test]
async fn test_set_state() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("PATCH", "/v1/gate/identities/idn_001/state")
        .match_body(r#"{"state":"inactive"}"#)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"id":"idn_001","schema_id":"user","state":"inactive","traits":{"email":"user@example.com","tenant_id":"ten_001"}}"#,
        )
        .create_async()
        .await;

    let gate = Gate::builder()
        .api_key("vrn_gate_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    let identity = gate
        .identities()
        .set_state("idn_001", "inactive")
        .await
        .unwrap();

    assert_eq!(identity.state, "inactive");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_activate_sends_state_active() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("PATCH", "/v1/gate/identities/idn_001/state")
        .match_body(r#"{"state":"active"}"#)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(identity_json("idn_001"))
        .create_async()
        .await;

    let gate = Gate::builder()
        .api_key("vrn_gate_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    gate.identities().activate("idn_001").await.unwrap();
    mock.assert_async().await;
}

#[tokio::test]
async fn test_resend_verification() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("POST", "/v1/gate/identities/idn_001/resend-verification")
        .with_status(204)
        .create_async()
        .await;

    let gate = Gate::builder()
        .api_key("vrn_gate_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    gate.identities()
        .resend_verification("idn_001")
        .await
        .unwrap();
    mock.assert_async().await;
}

#[tokio::test]
async fn test_get_security_settings() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/v1/gate/settings/security")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"passwordless_enabled":true,"mfa_enabled":false}"#)
        .create_async()
        .await;

    let gate = Gate::builder()
        .api_key("vrn_gate_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    let settings = gate.settings().get_security().await.unwrap();

    assert!(settings.passwordless_enabled);
    assert!(!settings.mfa_enabled);
    mock.assert_async().await;
}

#[tokio::test]
async fn test_update_security_settings() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("PUT", "/v1/gate/settings/security")
        .match_body(r#"{"passwordless_enabled":true,"mfa_enabled":true}"#)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"status":"ok"}"#)
        .create_async()
        .await;

    let gate = Gate::builder()
        .api_key("vrn_gate_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    gate.settings()
        .update_security(SecuritySettings {
            passwordless_enabled: true,
            mfa_enabled: true,
        })
        .await
        .unwrap();
    mock.assert_async().await;
}

#[tokio::test]
async fn test_create_token_sends_no_auth_header() {
    let mut server = Server::new_async().await;

    // Verify no Authorization header is sent
    let mock = server
        .mock("POST", "/v1/gate/tokens")
        .match_header("Authorization", mockito::Matcher::Missing)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"access_token":"tok_abc","expires_at":"2026-01-01T01:00:00Z","subject":"usr_123","tenant_id":"ten_001"}"#)
        .create_async()
        .await;

    let gate = Gate::builder()
        .api_key("vrn_gate_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    let token = gate
        .tokens()
        .create(CreateTokenParams {
            subject: "usr_123".into(),
            scopes: Some(vec!["gate.tokens.read".into()]),
            ttl_seconds: None,
        })
        .await
        .unwrap();

    assert_eq!(token.access_token, "tok_abc");
    assert_eq!(token.subject, "usr_123");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_introspect_token() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("POST", "/v1/gate/tokens/introspect")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"active":true,"subject":"usr_123","tenant_id":"ten_001","scopes":["gate.tokens.read"],"expires_at":"2026-01-01T01:00:00Z"}"#)
        .create_async()
        .await;

    let gate = Gate::builder()
        .api_key("vrn_gate_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    let info = gate.tokens().introspect("tok_abc").await.unwrap();

    assert!(info.active);
    assert_eq!(info.subject, "usr_123");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_authorize() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("POST", "/v1/gate/authorize")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"allowed":true,"decision_id":"dec_001","reason":"policy matched"}"#)
        .create_async()
        .await;

    let gate = Gate::builder()
        .api_key("vrn_gate_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    let decision = gate
        .authorize(AuthorizeParams {
            subject: "usr_123".into(),
            action: "relay.messages.read".into(),
            resource: "tenant:ten_001".into(),
            context: None,
        })
        .await
        .unwrap();

    assert!(decision.allowed);
    assert_eq!(decision.decision_id, "dec_001");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_gate_api_error() {
    let mut server = Server::new_async().await;

    server
        .mock("GET", "/v1/gate/identities/idn_notfound")
        .with_status(404)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error":{"code":"not_found","message":"Identity not found.","request_id":"req_404"}}"#)
        .create_async()
        .await;

    let gate = Gate::builder()
        .api_key("vrn_gate_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    let err = gate.identities().get("idn_notfound").await.unwrap_err();

    match err {
        nautilus::Error::Api(e) => {
            assert_eq!(e.code, "not_found");
            assert_eq!(e.status, 404);
        }
        other => panic!("expected Error::Api, got {other:?}"),
    }
}
