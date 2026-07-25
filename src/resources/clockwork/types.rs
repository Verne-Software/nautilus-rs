/// A scheduled cron job returned by the Clockwork API.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CronJob {
    /// Unique cron job identifier.
    pub id: String,
    /// Identifier of the tenant that owns the job.
    pub tenant_id: String,
    /// Human-readable name for the job.
    pub name: String,
    /// Cron expression describing the schedule (e.g. `"0 * * * *"`).
    pub schedule: String,
    /// URL invoked on each run.
    pub url: String,
    /// HTTP method used for the request (e.g. `"POST"`).
    pub method: String,
    /// Optional headers sent with each request.
    pub headers: Option<serde_json::Value>,
    /// Optional request body sent with each run.
    pub body: Option<String>,
    /// Whether the job is currently active.
    pub is_active: bool,
    /// ISO 8601 timestamp of the last run, if any.
    pub last_run_at: Option<String>,
    /// ISO 8601 timestamp of the next scheduled run, if any.
    pub next_run_at: Option<String>,
    /// ISO 8601 timestamp of when the job was created.
    pub created_at: String,
    /// ISO 8601 timestamp of when the job was last updated.
    pub updated_at: String,
}

/// A one-off delayed job returned by the Clockwork API.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DelayedJob {
    /// Unique delayed job identifier.
    pub id: String,
    /// Identifier of the tenant that owns the job.
    pub tenant_id: String,
    /// Human-readable name for the job.
    pub name: String,
    /// ISO 8601 timestamp at which the job runs.
    pub run_at: String,
    /// URL invoked when the job runs.
    pub url: String,
    /// HTTP method used for the request (e.g. `"POST"`).
    pub method: String,
    /// Optional headers sent with the request.
    pub headers: Option<serde_json::Value>,
    /// Optional request body sent with the run.
    pub body: Option<String>,
    /// Current status of the job (`"pending"`, `"completed"`, `"cancelled"`, …).
    pub status: String,
    /// ISO 8601 timestamp of when the job was created.
    pub created_at: String,
    /// ISO 8601 timestamp of when the job was last updated.
    pub updated_at: String,
}

/// A single execution record for a cron or delayed job.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Execution {
    /// Unique execution identifier.
    pub id: String,
    /// Identifier of the job that produced this execution.
    pub job_id: String,
    /// Status of the execution (`"success"`, `"failed"`, `"running"`, …).
    pub status: String,
    /// ISO 8601 timestamp of when the execution started.
    pub started_at: String,
    /// ISO 8601 timestamp of when the execution completed, if it has.
    pub completed_at: Option<String>,
    /// Total duration of the execution in milliseconds, if completed.
    pub duration_ms: Option<u64>,
    /// HTTP status code returned by the target, if the request completed.
    pub response_status: Option<u16>,
    /// Response body returned by the target, if captured.
    pub response_body: Option<String>,
    /// Error message, if the execution failed.
    pub error_message: Option<String>,
}

/// Parameters for [`CronJobsClient::create`](super::CronJobsClient::create).
///
/// # Example
///
/// ```no_run
/// use nautilus_rs::{Clockwork, CreateCronJobParams};
///
/// # async fn run() -> Result<(), nautilus_rs::Error> {
/// let clockwork = Clockwork::new("vrn_clockwork_live_sk_…");
/// let job = clockwork.jobs().create(CreateCronJobParams {
///     name: "nightly-report".into(),
///     schedule: "0 2 * * *".into(),
///     url: "https://example.com/hooks/report".into(),
///     ..Default::default()
/// }).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct CreateCronJobParams {
    /// Human-readable name for the job.
    pub name: String,
    /// Cron expression describing the schedule (e.g. `"0 * * * *"`).
    pub schedule: String,
    /// URL to invoke on each run.
    pub url: String,
    /// HTTP method used for the request (server default applies when `None`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// Optional headers sent with each request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<serde_json::Value>,
    /// Optional request body sent with each run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

/// Parameters for [`CronJobsClient::update`](super::CronJobsClient::update).
///
/// Every field is optional — only the fields you set are sent, and the rest are
/// left unchanged.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct UpdateCronJobParams {
    /// New name for the job.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// New cron expression describing the schedule.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    /// New URL to invoke on each run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// New HTTP method used for the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// New headers sent with each request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<serde_json::Value>,
    /// New request body sent with each run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// Activate or deactivate the job.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_active: Option<bool>,
}

/// Parameters for [`DelayedJobsClient::create`](super::DelayedJobsClient::create).
///
/// # Example
///
/// ```no_run
/// use nautilus_rs::{Clockwork, CreateDelayedJobParams};
///
/// # async fn run() -> Result<(), nautilus_rs::Error> {
/// let clockwork = Clockwork::new("vrn_clockwork_live_sk_…");
/// let job = clockwork.delayed().create(CreateDelayedJobParams {
///     name: "send-reminder".into(),
///     run_at: "2026-01-01T12:00:00Z".into(),
///     url: "https://example.com/hooks/reminder".into(),
///     ..Default::default()
/// }).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct CreateDelayedJobParams {
    /// Human-readable name for the job.
    pub name: String,
    /// ISO 8601 timestamp at which to run the job.
    pub run_at: String,
    /// URL to invoke when the job runs.
    pub url: String,
    /// HTTP method used for the request (server default applies when `None`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// Optional headers sent with the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<serde_json::Value>,
    /// Optional request body sent with the run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}
