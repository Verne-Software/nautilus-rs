use mockito::Server;
use nautilus_rs::{Clockwork, CreateCronJobParams, CreateDelayedJobParams, UpdateCronJobParams};

fn cron_job_json(id: &str) -> String {
    format!(
        r#"{{"id":"{id}","tenant_id":"ten_001","name":"nightly","schedule":"0 2 * * *","url":"https://example.com/hook","method":"POST","headers":null,"body":null,"is_active":true,"last_run_at":null,"next_run_at":"2026-01-02T02:00:00Z","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z"}}"#
    )
}

fn delayed_job_json(id: &str) -> String {
    format!(
        r#"{{"id":"{id}","tenant_id":"ten_001","name":"reminder","run_at":"2026-01-01T12:00:00Z","url":"https://example.com/hook","method":"POST","headers":null,"body":null,"status":"pending","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z"}}"#
    )
}

fn execution_json(id: &str, job_id: &str) -> String {
    format!(
        r#"{{"id":"{id}","job_id":"{job_id}","status":"success","started_at":"2026-01-01T02:00:00Z","completed_at":"2026-01-01T02:00:01Z","duration_ms":1200,"response_status":200,"response_body":"ok","error_message":null}}"#
    )
}

#[tokio::test]
async fn test_list_jobs() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/v1/clockwork/jobs")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(format!("[{}]", cron_job_json("job_001")))
        .create_async()
        .await;

    let clockwork = Clockwork::builder()
        .api_key("vrn_9f8a7c1b2d3e4f50")
        .base_url(server.url())
        .build()
        .unwrap();

    let jobs = clockwork.jobs().list().await.unwrap();

    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].id, "job_001");
    assert_eq!(jobs[0].schedule, "0 2 * * *");
    assert!(jobs[0].is_active);
    mock.assert_async().await;
}

#[tokio::test]
async fn test_create_job() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("POST", "/v1/clockwork/jobs")
        .with_status(201)
        .with_header("content-type", "application/json")
        .with_body(cron_job_json("job_001"))
        .create_async()
        .await;

    let clockwork = Clockwork::builder()
        .api_key("vrn_9f8a7c1b2d3e4f50")
        .base_url(server.url())
        .build()
        .unwrap();

    let job = clockwork
        .jobs()
        .create(CreateCronJobParams {
            name: "nightly".into(),
            schedule: "0 2 * * *".into(),
            url: "https://example.com/hook".into(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(job.id, "job_001");
    assert_eq!(job.name, "nightly");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_update_job() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("PATCH", "/v1/clockwork/jobs/job_001")
        .match_body(r#"{"schedule":"*/5 * * * *","is_active":false}"#)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(cron_job_json("job_001"))
        .create_async()
        .await;

    let clockwork = Clockwork::builder()
        .api_key("vrn_9f8a7c1b2d3e4f50")
        .base_url(server.url())
        .build()
        .unwrap();

    let job = clockwork
        .jobs()
        .update(
            "job_001",
            UpdateCronJobParams {
                schedule: Some("*/5 * * * *".into()),
                is_active: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    assert_eq!(job.id, "job_001");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_delete_job() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("DELETE", "/v1/clockwork/jobs/job_001")
        .with_status(204)
        .create_async()
        .await;

    let clockwork = Clockwork::builder()
        .api_key("vrn_9f8a7c1b2d3e4f50")
        .base_url(server.url())
        .build()
        .unwrap();

    clockwork.jobs().delete("job_001").await.unwrap();
    mock.assert_async().await;
}

#[tokio::test]
async fn test_job_executions() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/v1/clockwork/jobs/job_001/executions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(format!("[{}]", execution_json("exec_001", "job_001")))
        .create_async()
        .await;

    let clockwork = Clockwork::builder()
        .api_key("vrn_9f8a7c1b2d3e4f50")
        .base_url(server.url())
        .build()
        .unwrap();

    let executions = clockwork.jobs().executions("job_001").await.unwrap();

    assert_eq!(executions.len(), 1);
    assert_eq!(executions[0].id, "exec_001");
    assert_eq!(executions[0].job_id, "job_001");
    assert_eq!(executions[0].status, "success");
    assert_eq!(executions[0].duration_ms, Some(1200));
    assert_eq!(executions[0].response_status, Some(200));
    mock.assert_async().await;
}

#[tokio::test]
async fn test_list_delayed() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/v1/clockwork/delayed")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(format!("[{}]", delayed_job_json("dly_001")))
        .create_async()
        .await;

    let clockwork = Clockwork::builder()
        .api_key("vrn_9f8a7c1b2d3e4f50")
        .base_url(server.url())
        .build()
        .unwrap();

    let jobs = clockwork.delayed().list().await.unwrap();

    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].id, "dly_001");
    assert_eq!(jobs[0].status, "pending");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_create_delayed() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("POST", "/v1/clockwork/delayed")
        .with_status(201)
        .with_header("content-type", "application/json")
        .with_body(delayed_job_json("dly_001"))
        .create_async()
        .await;

    let clockwork = Clockwork::builder()
        .api_key("vrn_9f8a7c1b2d3e4f50")
        .base_url(server.url())
        .build()
        .unwrap();

    let job = clockwork
        .delayed()
        .create(CreateDelayedJobParams {
            name: "reminder".into(),
            run_at: "2026-01-01T12:00:00Z".into(),
            url: "https://example.com/hook".into(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(job.id, "dly_001");
    assert_eq!(job.run_at, "2026-01-01T12:00:00Z");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_cancel_delayed() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("DELETE", "/v1/clockwork/delayed/dly_001")
        .with_status(204)
        .create_async()
        .await;

    let clockwork = Clockwork::builder()
        .api_key("vrn_9f8a7c1b2d3e4f50")
        .base_url(server.url())
        .build()
        .unwrap();

    clockwork.delayed().cancel("dly_001").await.unwrap();
    mock.assert_async().await;
}

#[tokio::test]
async fn test_delayed_executions() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/v1/clockwork/delayed/dly_001/executions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(format!("[{}]", execution_json("exec_002", "dly_001")))
        .create_async()
        .await;

    let clockwork = Clockwork::builder()
        .api_key("vrn_9f8a7c1b2d3e4f50")
        .base_url(server.url())
        .build()
        .unwrap();

    let executions = clockwork.delayed().executions("dly_001").await.unwrap();

    assert_eq!(executions.len(), 1);
    assert_eq!(executions[0].id, "exec_002");
    assert_eq!(executions[0].job_id, "dly_001");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_clockwork_api_error() {
    let mut server = Server::new_async().await;

    server
        .mock("GET", "/v1/clockwork/jobs/job_notfound/executions")
        .with_status(404)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"error":{"code":"not_found","message":"Job not found.","request_id":"req_404"}}"#,
        )
        .create_async()
        .await;

    let clockwork = Clockwork::builder()
        .api_key("vrn_9f8a7c1b2d3e4f50")
        .base_url(server.url())
        .build()
        .unwrap();

    let err = clockwork
        .jobs()
        .executions("job_notfound")
        .await
        .unwrap_err();

    match err {
        nautilus_rs::Error::Api(e) => {
            assert_eq!(e.code, "not_found");
            assert_eq!(e.status, 404);
            assert_eq!(e.request_id, "req_404");
        }
        other => panic!("expected Error::Api, got {other:?}"),
    }
}

#[tokio::test]
async fn test_usage() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/v1/clockwork/usage")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"plan":"free","tier":"free","cron":{"active":2,"limit":5},"delayed":{"pending":0,"limit":null}}"#)
        .create_async()
        .await;

    let clockwork = Clockwork::builder()
        .api_key("vrn_9f8a7c1b2d3e4f50")
        .base_url(server.url())
        .build()
        .unwrap();

    let usage = clockwork.usage().await.unwrap();

    assert_eq!(usage.plan, "free");
    assert_eq!(usage.tier.as_deref(), Some("free"));
    assert_eq!(usage.cron.active, 2);
    assert_eq!(usage.cron.limit, Some(5));
    assert_eq!(usage.delayed.pending, 0);
    assert_eq!(usage.delayed.limit, None);
    mock.assert_async().await;
}

#[tokio::test]
async fn test_execution_attempt() {
    let mut server = Server::new_async().await;

    server
        .mock("GET", "/v1/clockwork/jobs/job_1/executions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[{"id":"e1","job_id":"job_1","status":"failed","started_at":"2026-10-14T10:00:00Z","completed_at":null,"duration_ms":null,"response_status":null,"response_body":null,"error_message":"timeout","attempt":2},{"id":"e0","job_id":"job_1","status":"success","started_at":"2026-10-13T10:00:00Z","completed_at":null,"duration_ms":null,"response_status":200,"response_body":null,"error_message":null}]"#)
        .create_async()
        .await;

    let clockwork = Clockwork::builder()
        .api_key("vrn_9f8a7c1b2d3e4f50")
        .base_url(server.url())
        .build()
        .unwrap();

    let executions = clockwork.jobs().executions("job_1").await.unwrap();

    assert_eq!(executions[0].attempt, Some(2));
    // Rows from before the field existed simply have none.
    assert_eq!(executions[1].attempt, None);
}

#[tokio::test]
async fn test_create_job_with_timezone() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("POST", "/v1/clockwork/jobs")
        .match_body(mockito::Matcher::Json(serde_json::json!({
            "name": "morning",
            "schedule": "0 9 * * *",
            "timezone": "Europe/Paris",
            "url": "https://example.com/hook"
        })))
        .with_status(201)
        .with_header("content-type", "application/json")
        .with_body(
            cron_job_json("job_tz")
                .replace(
                    r#""schedule":"0 2 * * *""#,
                    r#""schedule":"0 9 * * *","timezone":"Europe/Paris""#,
                )
                .replace("2026-01-02T02:00:00Z", "2026-10-16T07:00:00Z"),
        )
        .create_async()
        .await;

    let clockwork = Clockwork::builder()
        .api_key("vrn_9f8a7c1b2d3e4f50")
        .base_url(server.url())
        .build()
        .unwrap();

    let job = clockwork
        .jobs()
        .create(CreateCronJobParams {
            name: "morning".into(),
            schedule: "0 9 * * *".into(),
            timezone: Some("Europe/Paris".into()),
            url: "https://example.com/hook".into(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(job.timezone, "Europe/Paris");
    assert_eq!(job.next_run_at.as_deref(), Some("2026-10-16T07:00:00Z"));
    mock.assert_async().await;
}

#[tokio::test]
async fn test_update_job_timezone_sends_only_the_zone() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("PATCH", "/v1/clockwork/jobs/job_001")
        .match_body(r#"{"timezone":"America/New_York"}"#)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            cron_job_json("job_001")
                .replace(r#""url":"#, r#""timezone":"America/New_York","url":"#),
        )
        .create_async()
        .await;

    let clockwork = Clockwork::builder()
        .api_key("vrn_9f8a7c1b2d3e4f50")
        .base_url(server.url())
        .build()
        .unwrap();

    let job = clockwork
        .jobs()
        .update(
            "job_001",
            UpdateCronJobParams {
                timezone: Some("America/New_York".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    assert_eq!(job.timezone, "America/New_York");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_a_job_without_timezone_in_the_answer_is_utc() {
    let mut server = Server::new_async().await;

    let _mock = server
        .mock("GET", "/v1/clockwork/jobs")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(format!("[{}]", cron_job_json("job_001")))
        .create_async()
        .await;

    let clockwork = Clockwork::builder()
        .api_key("vrn_9f8a7c1b2d3e4f50")
        .base_url(server.url())
        .build()
        .unwrap();

    assert_eq!(clockwork.jobs().list().await.unwrap()[0].timezone, "UTC");
}

#[tokio::test]
async fn test_an_unknown_timezone_is_400_invalid_payload() {
    let mut server = Server::new_async().await;

    let _mock = server
        .mock("POST", "/v1/clockwork/jobs")
        .with_status(400)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error":{"code":"invalid_payload","message":"Unknown timezone \"Mars/Olympus\".","request_id":"req_tz"}}"#)
        .create_async()
        .await;

    let clockwork = Clockwork::builder()
        .api_key("vrn_9f8a7c1b2d3e4f50")
        .base_url(server.url())
        .build()
        .unwrap();

    let err = clockwork
        .jobs()
        .create(CreateCronJobParams {
            name: "x".into(),
            schedule: "0 9 * * *".into(),
            timezone: Some("Mars/Olympus".into()),
            url: "https://example.com".into(),
            ..Default::default()
        })
        .await
        .unwrap_err();

    match err {
        nautilus_rs::Error::Api(e) => {
            assert_eq!(e.status, 400);
            assert_eq!(e.code, "invalid_payload");
        }
        other => panic!("expected an API error, got {other:?}"),
    }
}
