use nautilus_rs::{Error, Verne};

#[tokio::test]
async fn test_builder_relay_only() {
    let verne = Verne::builder()
        .relay("vrn_9f8a7c1b2d3e4f50")
        .build()
        .unwrap();

    assert!(verne.relay().is_ok());
    assert!(verne.gate().is_err());
}

#[tokio::test]
async fn test_builder_gate_only() {
    let verne = Verne::builder()
        .gate("vrn_9f8a7c1b2d3e4f50")
        .build()
        .unwrap();

    assert!(verne.gate().is_ok());
    assert!(verne.relay().is_err());
}

#[tokio::test]
async fn test_builder_both_services() {
    let verne = Verne::builder()
        .relay("vrn_9f8a7c1b2d3e4f50")
        .gate("vrn_9f8a7c1b2d3e4f50")
        .build()
        .unwrap();

    assert!(verne.relay().is_ok());
    assert!(verne.gate().is_ok());
}

#[tokio::test]
async fn test_builder_clockwork_only() {
    let verne = Verne::builder()
        .clockwork("vrn_9f8a7c1b2d3e4f50")
        .build()
        .unwrap();

    assert!(verne.clockwork().is_ok());
    assert!(verne.relay().is_err());
}

#[tokio::test]
async fn test_missing_relay_key_returns_config_error() {
    let verne = Verne::builder().build().unwrap();

    let err = verne.relay().unwrap_err();
    match err {
        Error::Config(msg) => assert!(!msg.is_empty()),
        other => panic!("expected Error::Config, got {other:?}"),
    }
}

#[tokio::test]
async fn test_missing_gate_key_returns_config_error() {
    let verne = Verne::builder().build().unwrap();

    let err = verne.gate().unwrap_err();
    match err {
        Error::Config(msg) => assert!(!msg.is_empty()),
        other => panic!("expected Error::Config, got {other:?}"),
    }
}

#[tokio::test]
async fn test_missing_clockwork_key_returns_config_error() {
    let verne = Verne::builder().build().unwrap();

    let err = verne.clockwork().unwrap_err();
    match err {
        Error::Config(msg) => assert!(!msg.is_empty()),
        other => panic!("expected Error::Config, got {other:?}"),
    }
}

#[tokio::test]
async fn test_builder_with_custom_timeout() {
    let verne = Verne::builder()
        .relay("vrn_9f8a7c1b2d3e4f50")
        .timeout_secs(60)
        .build()
        .unwrap();

    assert!(verne.relay().is_ok());
}

// --- one key for every service ---------------------------------------------------------

mod one_key {
    use mockito::Server;
    use nautilus_rs::{Error, Verne};

    const KEY: &str = "vrn_9f8a7c1b2d3e4f50";

    #[tokio::test]
    async fn api_key_serves_every_service() {
        let verne = Verne::builder().api_key(KEY).build().unwrap();

        assert!(verne.relay().is_ok());
        assert!(verne.gate().is_ok());
        assert!(verne.passepartout().is_ok());
        assert!(verne.clockwork().is_ok());
    }

    #[tokio::test]
    async fn api_key_is_sent_as_the_bearer_token_by_each_service() {
        let mut server = Server::new_async().await;
        let bearer = format!("Bearer {KEY}");
        let clockwork = server
            .mock("GET", "/v1/clockwork/jobs")
            .match_header("authorization", bearer.as_str())
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("[]")
            .create_async()
            .await;
        let gate = server
            .mock("GET", "/v1/gate/settings/return-urls")
            .match_header("authorization", bearer.as_str())
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"return_urls":[]}"#)
            .create_async()
            .await;

        let verne = Verne::builder()
            .api_key(KEY)
            .base_url(server.url())
            .build()
            .unwrap();
        verne.clockwork().unwrap().jobs().list().await.unwrap();
        verne
            .gate()
            .unwrap()
            .settings()
            .get_return_urls()
            .await
            .unwrap();

        clockwork.assert_async().await;
        gate.assert_async().await;
    }

    /// A per-service key overrides `api_key` for that service only.
    #[tokio::test]
    async fn a_per_service_key_overrides_api_key_for_that_service_only() {
        let mut server = Server::new_async().await;
        let overridden = server
            .mock("GET", "/v1/clockwork/jobs")
            .match_header("authorization", "Bearer vrn_other")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("[]")
            .create_async()
            .await;
        let shared = server
            .mock("GET", "/v1/gate/identities/u")
            .match_header("authorization", format!("Bearer {KEY}").as_str())
            .with_status(404)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error":{"code":"not_found","message":"No.","request_id":"r"}}"#)
            .create_async()
            .await;

        let verne = Verne::builder()
            .api_key(KEY)
            .clockwork("vrn_other")
            .base_url(server.url())
            .build()
            .unwrap();
        verne.clockwork().unwrap().jobs().list().await.unwrap();
        let _ = verne.gate().unwrap().identities().get("u").await;

        overridden.assert_async().await;
        shared.assert_async().await;
    }

    #[tokio::test]
    async fn per_service_keys_alone_still_work_as_in_1_3() {
        let verne = Verne::builder()
            .relay("vrn_relay_9f8a7c1b2d3e4f50")
            .gate("vrn_gate_9f8a7c1b2d3e4f50")
            .build()
            .unwrap();

        assert!(verne.relay().is_ok());
        assert!(verne.gate().is_ok());
        assert!(verne.passepartout().is_err());
    }

    #[tokio::test]
    async fn with_no_key_the_error_names_api_key_first() {
        let verne = Verne::builder().build().unwrap();

        match verne.gate().unwrap_err() {
            Error::Config(msg) => {
                assert!(msg.contains("`api_key`"), "{msg}");
                assert!(msg.find("api_key").unwrap() < msg.find("`gate`").unwrap());
            }
            other => panic!("expected Error::Config, got {other:?}"),
        }
    }
}
