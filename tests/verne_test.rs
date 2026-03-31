use vernesoft::{Error, Verne};

#[tokio::test]
async fn test_builder_relay_only() {
    let verne = Verne::builder()
        .relay("vrn_relay_test_sk_abc")
        .build()
        .unwrap();

    assert!(verne.relay().is_ok());
    assert!(verne.gate().is_err());
}

#[tokio::test]
async fn test_builder_gate_only() {
    let verne = Verne::builder()
        .gate("vrn_gate_test_sk_abc")
        .build()
        .unwrap();

    assert!(verne.gate().is_ok());
    assert!(verne.relay().is_err());
}

#[tokio::test]
async fn test_builder_both_services() {
    let verne = Verne::builder()
        .relay("vrn_relay_test_sk_abc")
        .gate("vrn_gate_test_sk_abc")
        .build()
        .unwrap();

    assert!(verne.relay().is_ok());
    assert!(verne.gate().is_ok());
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
async fn test_builder_with_custom_timeout() {
    let verne = Verne::builder()
        .relay("vrn_relay_test_sk_abc")
        .timeout_secs(60)
        .build()
        .unwrap();

    assert!(verne.relay().is_ok());
}
