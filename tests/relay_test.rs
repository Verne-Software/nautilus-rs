use mockito::Server;
use vernesoft::{ListMessagesParams, Relay, SendMessageParams};

#[tokio::test]
async fn test_send_message() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("POST", "/v1/relay/messages")
        .with_status(202)
        .with_header("content-type", "application/json")
        .with_body(r#"{"id":"msg_001","event_type":"user.created","status":"accepted","timestamp":"2026-01-01T00:00:00Z"}"#)
        .create_async()
        .await;

    let relay = Relay::builder()
        .api_key("vrn_relay_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    let msg = relay
        .messages()
        .send(SendMessageParams {
            event_type: "user.created".into(),
            payload: serde_json::json!({"id": "123"}),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(msg.id, "msg_001");
    assert_eq!(msg.event_type, "user.created");
    assert_eq!(msg.status, "accepted");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_send_message_with_all_params() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("POST", "/v1/relay/messages")
        .with_status(202)
        .with_header("content-type", "application/json")
        .with_body(r#"{"id":"msg_002","event_type":"order.placed","status":"accepted","timestamp":"2026-01-01T00:00:00Z"}"#)
        .create_async()
        .await;

    let relay = Relay::builder()
        .api_key("vrn_relay_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    let msg = relay
        .messages()
        .send(SendMessageParams {
            event_type: "order.placed".into(),
            payload: serde_json::json!({"order_id": "ord_999"}),
            idempotency_key: Some("idem_001".into()),
            channels: Some(vec!["team-a".into(), "team-b".into()]),
        })
        .await
        .unwrap();

    assert_eq!(msg.id, "msg_002");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_list_messages() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/v1/relay/messages?limit=10&event_type=user.created")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"data":[{"id":"msg_001","event_type":"user.created","status":"accepted","timestamp":"2026-01-01T00:00:00Z"}],"has_more":false,"next_cursor":null}"#)
        .create_async()
        .await;

    let relay = Relay::builder()
        .api_key("vrn_relay_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    let page = relay
        .messages()
        .list(ListMessagesParams {
            limit: Some(10),
            event_type: Some("user.created".into()),
            cursor: None,
        })
        .await
        .unwrap();

    assert_eq!(page.data.len(), 1);
    assert!(!page.has_more);
    assert_eq!(page.data[0].id, "msg_001");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_send_message_api_error() {
    let mut server = Server::new_async().await;

    server
        .mock("POST", "/v1/relay/messages")
        .with_status(400)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error":{"code":"invalid_payload","message":"Field 'event_type' is required.","request_id":"req_abc123"}}"#)
        .create_async()
        .await;

    let relay = Relay::builder()
        .api_key("vrn_relay_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    let err = relay
        .messages()
        .send(SendMessageParams {
            event_type: "".into(),
            payload: serde_json::json!({}),
            ..Default::default()
        })
        .await
        .unwrap_err();

    match err {
        vernesoft::Error::Api(e) => {
            assert_eq!(e.code, "invalid_payload");
            assert_eq!(e.status, 400);
            assert_eq!(e.request_id, "req_abc123");
        }
        other => panic!("expected Error::Api, got {other:?}"),
    }
}

#[tokio::test]
async fn test_send_message_retries_on_429() {
    let mut server = Server::new_async().await;

    // First call: 429
    let mock_429 = server
        .mock("POST", "/v1/relay/messages")
        .with_status(429)
        .with_header("content-type", "application/json")
        .with_header("Retry-After", "0")
        .with_body(r#"{"error":{"code":"rate_limited","message":"Too many requests.","request_id":"req_rate1"}}"#)
        .create_async()
        .await;

    // Second call: success
    let mock_ok = server
        .mock("POST", "/v1/relay/messages")
        .with_status(202)
        .with_header("content-type", "application/json")
        .with_body(r#"{"id":"msg_retry","event_type":"user.created","status":"accepted","timestamp":"2026-01-01T00:00:00Z"}"#)
        .create_async()
        .await;

    let relay = Relay::builder()
        .api_key("vrn_relay_test_sk_abc")
        .base_url(server.url())
        .build()
        .unwrap();

    let msg = relay
        .messages()
        .send(SendMessageParams {
            event_type: "user.created".into(),
            payload: serde_json::json!({"id": "123"}),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(msg.id, "msg_retry");
    mock_429.assert_async().await;
    mock_ok.assert_async().await;
}
