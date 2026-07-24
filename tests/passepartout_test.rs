use mockito::Server;
use nautilus_rs::Passepartout;

#[tokio::test]
async fn test_login_start() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("POST", "/v1/passepartout/login/start")
        .match_header("Authorization", "Bearer vrn_passepartout_test_sk_abc")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"nonce":"nonce_abc","deep_link":"https://t.me/VerneBot?start=nonce_abc","expires_at":"2026-01-01T01:00:00Z"}"#,
        )
        .create_async()
        .await;

    let passepartout = Passepartout::builder()
        .api_key("vrn_passepartout_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    let flow = passepartout.login_start().await.unwrap();

    assert_eq!(flow.nonce, "nonce_abc");
    assert_eq!(flow.deep_link, "https://t.me/VerneBot?start=nonce_abc");
    assert_eq!(flow.expires_at, "2026-01-01T01:00:00Z");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_login_status_pending() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/v1/passepartout/login/status?nonce=nonce_abc")
        .match_header("Authorization", "Bearer vrn_passepartout_test_sk_abc")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"status":"pending"}"#)
        .create_async()
        .await;

    let passepartout = Passepartout::builder()
        .api_key("vrn_passepartout_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    let status = passepartout.login_status("nonce_abc").await.unwrap();

    assert_eq!(status.status, "pending");
    assert!(status.access_token.is_none());
    assert!(status.user.is_none());
    mock.assert_async().await;
}

#[tokio::test]
async fn test_login_status_completed() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/v1/passepartout/login/status?nonce=nonce_abc")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"status":"completed","access_token":"tok_abc","expires_at":"2026-01-01T01:00:00Z","identity_id":"idn_001","user":{"id":"12345","username":"alice","first_name":"Alice","photo_url":"https://t.me/i/photo.jpg"}}"#,
        )
        .create_async()
        .await;

    let passepartout = Passepartout::builder()
        .api_key("vrn_passepartout_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    let status = passepartout.login_status("nonce_abc").await.unwrap();

    assert_eq!(status.status, "completed");
    assert_eq!(status.access_token.as_deref(), Some("tok_abc"));
    assert_eq!(status.identity_id.as_deref(), Some("idn_001"));
    let user = status.user.expect("user should be present");
    assert_eq!(user.id, "12345");
    assert_eq!(user.username.as_deref(), Some("alice"));
    assert_eq!(user.first_name.as_deref(), Some("Alice"));
    assert_eq!(user.photo_url.as_deref(), Some("https://t.me/i/photo.jpg"));
    mock.assert_async().await;
}

#[tokio::test]
async fn test_login_status_urlencodes_nonce() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/v1/passepartout/login/status?nonce=a%20b%2Fc")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"status":"pending"}"#)
        .create_async()
        .await;

    let passepartout = Passepartout::builder()
        .api_key("vrn_passepartout_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    let status = passepartout.login_status("a b/c").await.unwrap();

    assert_eq!(status.status, "pending");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_introspect() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("POST", "/v1/passepartout/tokens/introspect")
        .match_header("Authorization", "Bearer vrn_passepartout_test_sk_abc")
        .match_body(r#"{"access_token":"tok_abc"}"#)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"active":true,"subject":"idn_001","tenant_id":"ten_001","scopes":["passepartout.login"],"expires_at":"2026-01-01T01:00:00Z"}"#,
        )
        .create_async()
        .await;

    let passepartout = Passepartout::builder()
        .api_key("vrn_passepartout_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    let info = passepartout.introspect("tok_abc").await.unwrap();

    assert!(info.active);
    assert_eq!(info.subject.as_deref(), Some("idn_001"));
    assert_eq!(info.tenant_id.as_deref(), Some("ten_001"));
    assert_eq!(
        info.scopes,
        Some(vec!["passepartout.login".to_string()])
    );
    assert_eq!(info.expires_at.as_deref(), Some("2026-01-01T01:00:00Z"));
    mock.assert_async().await;
}

#[tokio::test]
async fn test_introspect_inactive() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("POST", "/v1/passepartout/tokens/introspect")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"active":false}"#)
        .create_async()
        .await;

    let passepartout = Passepartout::builder()
        .api_key("vrn_passepartout_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    let info = passepartout.introspect("tok_bad").await.unwrap();

    assert!(!info.active);
    assert!(info.subject.is_none());
    assert!(info.scopes.is_none());
    mock.assert_async().await;
}

#[tokio::test]
async fn test_passepartout_via_verne() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("POST", "/v1/passepartout/login/start")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"nonce":"nonce_xyz","deep_link":"https://t.me/VerneBot?start=nonce_xyz","expires_at":"2026-01-01T01:00:00Z"}"#,
        )
        .create_async()
        .await;

    let verne = nautilus_rs::Verne::builder()
        .passepartout("vrn_passepartout_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    let flow = verne.passepartout().unwrap().login_start().await.unwrap();

    assert_eq!(flow.nonce, "nonce_xyz");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_passepartout_api_error() {
    let mut server = Server::new_async().await;

    server
        .mock("GET", "/v1/passepartout/login/status?nonce=nonce_missing")
        .with_status(404)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"error":{"code":"not_found","message":"Login attempt not found.","request_id":"req_404"}}"#,
        )
        .create_async()
        .await;

    let passepartout = Passepartout::builder()
        .api_key("vrn_passepartout_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    let err = passepartout
        .login_status("nonce_missing")
        .await
        .unwrap_err();

    match err {
        nautilus_rs::Error::Api(e) => {
            assert_eq!(e.code, "not_found");
            assert_eq!(e.status, 404);
        }
        other => panic!("expected Error::Api, got {other:?}"),
    }
}
