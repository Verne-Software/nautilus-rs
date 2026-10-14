mod common;

use std::time::{Duration, Instant};

use common::{JSON, KEY};
use mockito::Server;
use nautilus_rs::{
    ApiError, Clockwork, CreateCronJobParams, Error, Gate, Passepartout, Relay, SendMessageParams,
};
use serde_json::json;

fn send() -> SendMessageParams {
    SendMessageParams {
        event_type: "ping".into(),
        payload: json!({}),
        ..Default::default()
    }
}

fn relay(server: &Server) -> Relay {
    Relay::builder()
        .api_key(KEY)
        .base_url(server.url())
        .build()
        .unwrap()
}

fn api(err: Error) -> ApiError {
    match err {
        Error::Api(e) => *e,
        other => panic!("expected Error::Api, got {other:?}"),
    }
}

// --- 429 -----------------------------------------------------------------------------

/// Relay's monthly limit answers `Retry-After` = seconds to the 1st of next month. Waiting
/// that out would block a request for weeks.
#[tokio::test]
async fn the_monthly_relay_limit_is_returned_at_once_with_retry_after_and_details() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/relay/messages")
        .with_status(429)
        .with_header("retry-after", "1209600")
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"error":{"code":"RELAY_MESSAGE_LIMIT_REACHED","message":"You have sent the 10000 messages the Free plan includes this month.","request_id":"req_1","plan":"free","limit":10000,"current":10000}}"#)
        .expect(1)
        .create_async()
        .await;

    let started = Instant::now();
    let err = api(relay(&server).messages().send(send()).await.unwrap_err());

    assert!(started.elapsed() < Duration::from_secs(1), "must not sleep");
    assert_eq!(err.status, 429);
    assert_eq!(err.code, "RELAY_MESSAGE_LIMIT_REACHED");
    assert_eq!(err.retry_after, Some(1_209_600));
    assert_eq!(err.details["plan"], "free");
    assert_eq!(err.details["limit"], 10000);
    assert_eq!(err.details["current"], 10000);
    mock.assert_async().await;
}

/// Even a short `Retry-After` is not waited out for the monthly-limit code.
#[tokio::test]
async fn the_monthly_limit_code_is_never_retried_even_with_a_short_retry_after() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/relay/messages")
        .with_status(429)
        .with_header("retry-after", "1")
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"error":{"code":"RELAY_MESSAGE_LIMIT_REACHED","message":"limit","request_id":"r"}}"#)
        .expect(1)
        .create_async()
        .await;

    let err = api(relay(&server).messages().send(send()).await.unwrap_err());
    assert_eq!(err.retry_after, Some(1));
    mock.assert_async().await;
}

#[tokio::test]
async fn a_retry_after_over_a_minute_is_not_retried() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/relay/messages")
        .with_status(429)
        .with_header("retry-after", "61")
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"error":{"code":"rate_limited","message":"slow down","request_id":"r"}}"#)
        .expect(1)
        .create_async()
        .await;

    let err = api(relay(&server).messages().send(send()).await.unwrap_err());
    assert_eq!((err.status, err.retry_after), (429, Some(61)));
    mock.assert_async().await;
}

#[tokio::test]
async fn a_short_429_is_retried_once() {
    let mut server = Server::new_async().await;
    let first = server
        .mock("POST", "/v1/relay/messages")
        .with_status(429)
        .with_header("retry-after", "1")
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"error":{"code":"rate_limited","message":"slow down","request_id":"r"}}"#)
        .expect(1)
        .create_async()
        .await;
    let second = server
        .mock("POST", "/v1/relay/messages")
        .with_status(202)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"id":"msg_1","event_type":"ping","status":"accepted","timestamp":"2026-10-14T10:00:00Z"}"#)
        .expect(1)
        .create_async()
        .await;

    let started = Instant::now();
    let msg = relay(&server).messages().send(send()).await.unwrap();

    assert_eq!(msg.id, "msg_1");
    assert!(
        started.elapsed() >= Duration::from_secs(1),
        "waited Retry-After"
    );
    first.assert_async().await;
    second.assert_async().await;
}

/// Only one retry: a second `429` is returned.
#[tokio::test]
async fn a_second_429_is_returned_not_retried_again() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/relay/messages")
        .with_status(429)
        .with_header("retry-after", "0")
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"error":{"code":"rate_limited","message":"slow down","request_id":"r"}}"#)
        .expect(2)
        .create_async()
        .await;

    let err = api(relay(&server).messages().send(send()).await.unwrap_err());
    assert_eq!(err.status, 429);
    mock.assert_async().await;
}

#[tokio::test]
async fn a_429_on_a_get_is_not_retried() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/clockwork/jobs")
        .with_status(429)
        .with_header("retry-after", "0")
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"error":{"code":"rate_limited","message":"slow down","request_id":"r"}}"#)
        .expect(1)
        .create_async()
        .await;

    let cw = Clockwork::builder()
        .api_key(KEY)
        .base_url(server.url())
        .build()
        .unwrap();
    assert_eq!(api(cw.jobs().list().await.unwrap_err()).status, 429);
    mock.assert_async().await;
}

#[tokio::test]
async fn a_402_is_never_retried_and_keeps_its_limit_details() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/clockwork/jobs")
        .with_status(402)
        .with_header("retry-after", "0")
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"error":{"code":"UPGRADE_REQUIRED","message":"limit","request_id":"r","kind":"cron","limit":10,"current":10}}"#)
        .expect(1)
        .create_async()
        .await;

    let cw = Clockwork::builder()
        .api_key(KEY)
        .base_url(server.url())
        .build()
        .unwrap();
    let err = api(cw
        .jobs()
        .create(CreateCronJobParams {
            name: "n".into(),
            schedule: "0 * * * *".into(),
            url: "https://example.com".into(),
            ..Default::default()
        })
        .await
        .unwrap_err());

    assert_eq!((err.status, err.code.as_str()), (402, "UPGRADE_REQUIRED"));
    assert_eq!(err.details["kind"], "cron");
    mock.assert_async().await;
}

// --- error shapes --------------------------------------------------------------------

#[tokio::test]
async fn the_403_for_a_missing_scope_is_the_standard_envelope() {
    let mut server = Server::new_async().await;
    server
        .mock("POST", "/v1/passepartout/login/start")
        .with_status(403)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"error":{"code":"forbidden","message":"This API key is not authorized for Passepartout.","request_id":"req_x"}}"#)
        .create_async()
        .await;

    let pp = Passepartout::builder()
        .api_key(KEY)
        .base_url(server.url())
        .build()
        .unwrap();
    let err = api(pp.login_start().await.unwrap_err());

    assert_eq!(err.status, 403);
    assert_eq!(err.code, "forbidden");
    assert_eq!(
        err.message,
        "This API key is not authorized for Passepartout."
    );
    assert_eq!(err.request_id, "req_x");
    assert!(err.details.is_empty());
    assert_eq!(err.retry_after, None);
}

/// Clockwork engine errors are `{"error": "<sentence>"}`, a string.
#[tokio::test]
async fn a_plain_string_error_keeps_its_sentence_under_code_unknown() {
    let mut server = Server::new_async().await;
    server
        .mock("POST", "/v1/clockwork/jobs")
        .with_status(400)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"error":"Invalid cron schedule. Use five fields."}"#)
        .create_async()
        .await;

    let cw = Clockwork::builder()
        .api_key(KEY)
        .base_url(server.url())
        .build()
        .unwrap();
    let err = cw
        .jobs()
        .create(CreateCronJobParams {
            name: "n".into(),
            schedule: "nope".into(),
            url: "https://example.com".into(),
            ..Default::default()
        })
        .await
        .unwrap_err();
    // An Error::Api, not Error::Json or Error::Http.
    let err = api(err);

    assert_eq!(err.status, 400);
    assert_eq!(err.code, "unknown");
    assert_eq!(err.message, "Invalid cron schedule. Use five fields.");
    assert_eq!(err.request_id, "");
    assert_eq!(
        err.body,
        Some(json!({"error": "Invalid cron schedule. Use five fields."}))
    );
}

/// Kratos passthrough: `code` is a number, which is never used as the code.
#[tokio::test]
async fn a_numeric_code_is_never_the_code() {
    let mut server = Server::new_async().await;
    server
        .mock("DELETE", "/v1/gate/sessions/abc")
        .with_status(404)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"error":{"code":404,"status":"Not Found","message":"The requested resource could not be found"}}"#)
        .create_async()
        .await;

    let gate = Gate::builder()
        .api_key(KEY)
        .base_url(server.url())
        .build()
        .unwrap();
    let err = api(gate.sessions().revoke("abc").await.unwrap_err());

    assert_eq!(err.code, "unknown");
    assert_eq!(err.message, "The requested resource could not be found");
    // The numeric code and the status are kept in details, not lost.
    assert_eq!(err.details["status"], "Not Found");
}

/// `POST /v1/gate/tokens` 400: the code is `invalid_request`, there is no `request_id`.
#[tokio::test]
async fn an_envelope_without_request_id_is_fine() {
    let mut server = Server::new_async().await;
    server
        .mock("POST", "/v1/gate/tokens")
        .with_status(400)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"error":{"code":"invalid_request","message":"subject is required."}}"#)
        .create_async()
        .await;

    let gate = Gate::builder()
        .api_key(KEY)
        .base_url(server.url())
        .build()
        .unwrap();
    let err = api(gate
        .tokens()
        .create(nautilus_rs::CreateTokenParams {
            subject: String::new(),
            scopes: None,
            ttl_seconds: None,
        })
        .await
        .unwrap_err());

    assert_eq!(err.code, "invalid_request");
    assert_eq!(err.request_id, "");
    // Display does not print an empty request id.
    assert_eq!(err.to_string(), "[invalid_request] subject is required.");
}

#[tokio::test]
async fn a_body_that_is_not_json_is_still_an_api_error_with_the_status() {
    let mut server = Server::new_async().await;
    server
        .mock("GET", "/v1/gate/identities/x")
        .with_status(502)
        .with_header("content-type", "text/html")
        .with_body("<html>bad gateway</html>")
        .create_async()
        .await;

    let gate = Gate::builder()
        .api_key(KEY)
        .base_url(server.url())
        .build()
        .unwrap();
    let err = api(gate.identities().get("x").await.unwrap_err());

    assert_eq!(err.status, 502);
    assert_eq!(err.code, "unknown");
    assert_eq!(err.message, "HTTP 502");
    assert_eq!(err.body, None);
}

#[tokio::test]
async fn a_detail_list_or_bare_message_is_used_when_there_is_no_envelope() {
    let mut server = Server::new_async().await;
    server
        .mock("GET", "/v1/gate/identities/detail-list")
        .with_status(422)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"detail":[{"msg":"field required"},{"msg":"bad email"}]}"#)
        .create_async()
        .await;
    server
        .mock("GET", "/v1/gate/identities/bare")
        .with_status(500)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"message":"boom"}"#)
        .create_async()
        .await;
    server
        .mock("GET", "/v1/gate/identities/empty")
        .with_status(500)
        .with_body("")
        .create_async()
        .await;
    server
        .mock("GET", "/v1/gate/identities/array")
        .with_status(500)
        .with_body("[1,2]")
        .create_async()
        .await;

    let gate = Gate::builder()
        .api_key(KEY)
        .base_url(server.url())
        .build()
        .unwrap();

    let e = api(gate.identities().get("detail-list").await.unwrap_err());
    assert_eq!(
        (e.status, e.code.as_str(), e.message.as_str()),
        (422, "unknown", "field required; bad email")
    );
    let e = api(gate.identities().get("bare").await.unwrap_err());
    assert_eq!(e.message, "boom");
    let e = api(gate.identities().get("empty").await.unwrap_err());
    assert_eq!(e.message, "HTTP 500");
    let e = api(gate.identities().get("array").await.unwrap_err());
    assert_eq!(e.message, "HTTP 500");
}

#[tokio::test]
async fn retry_after_is_parsed_on_any_error_and_rounded_up() {
    let mut server = Server::new_async().await;
    server
        .mock("GET", "/v1/gate/identities/a")
        .with_status(503)
        .with_header("retry-after", "2.5")
        .with_header(JSON.0, JSON.1)
        .with_body(
            r#"{"error":{"code":"upstream_unavailable","message":"later","request_id":"r"}}"#,
        )
        .create_async()
        .await;
    server
        .mock("GET", "/v1/gate/identities/b")
        .with_status(503)
        .with_header("retry-after", "Wed, 21 Oct 2026 07:28:00 GMT")
        .with_header(JSON.0, JSON.1)
        .with_body(
            r#"{"error":{"code":"upstream_unavailable","message":"later","request_id":"r"}}"#,
        )
        .create_async()
        .await;

    let gate = Gate::builder()
        .api_key(KEY)
        .base_url(server.url())
        .build()
        .unwrap();
    assert_eq!(
        api(gate.identities().get("a").await.unwrap_err()).retry_after,
        Some(3)
    );
    // An HTTP-date is not understood; the gateway sends seconds. It reads as absent.
    assert_eq!(
        api(gate.identities().get("b").await.unwrap_err()).retry_after,
        None
    );
}

#[tokio::test]
async fn a_success_body_that_does_not_parse_is_a_json_error() {
    let mut server = Server::new_async().await;
    server
        .mock("GET", "/v1/gate/settings/security")
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"not":"settings"}"#)
        .create_async()
        .await;
    let gate = Gate::builder()
        .api_key(KEY)
        .base_url(server.url())
        .build()
        .unwrap();
    assert!(matches!(
        gate.settings().get_security().await.unwrap_err(),
        Error::Json(_)
    ));
}
