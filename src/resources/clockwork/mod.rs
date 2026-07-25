pub mod types;

use std::sync::Arc;

use crate::{error::Error, http::HttpClient};
use types::{
    CreateCronJobParams, CreateDelayedJobParams, CronJob, DelayedJob, Execution,
    UpdateCronJobParams,
};

/// Clockwork service client — Cron-as-a-Service.
///
/// Schedules recurring cron jobs and one-off delayed jobs that invoke your HTTP
/// endpoints, and reports on their execution history.
///
/// Obtain a `Clockwork` instance either as part of the unified [`Verne`] client
/// or standalone:
///
/// ```no_run
/// // Standalone
/// use nautilus_rs::Clockwork;
/// let clockwork = Clockwork::new("vrn_clockwork_live_sk_…");
///
/// // Via unified client
/// use nautilus_rs::Verne;
/// # fn run() -> Result<(), nautilus_rs::Error> {
/// let verne = Verne::builder().clockwork("vrn_clockwork_live_sk_…").build()?;
/// let clockwork = verne.clockwork()?;
/// # Ok(())
/// # }
/// ```
///
/// [`Verne`]: crate::Verne
pub struct Clockwork {
    http: Arc<HttpClient>,
}

impl std::fmt::Debug for Clockwork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Clockwork").finish_non_exhaustive()
    }
}

impl Clockwork {
    /// Create a `Clockwork` client with default settings.
    ///
    /// Panics if the API key is empty or the HTTP client cannot be
    /// initialised. Use [`Clockwork::builder`] for fallible construction.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::builder()
            .api_key(api_key)
            .build()
            .expect("failed to build Clockwork client")
    }

    /// Return a [`ClockworkBuilder`] for fine-grained configuration.
    pub fn builder() -> ClockworkBuilder {
        ClockworkBuilder::default()
    }

    pub(crate) fn from_http(http: Arc<HttpClient>) -> Self {
        Self { http }
    }

    /// Return a [`CronJobsClient`] for managing recurring cron jobs.
    pub fn jobs(&self) -> CronJobsClient {
        CronJobsClient {
            http: Arc::clone(&self.http),
        }
    }

    /// Return a [`DelayedJobsClient`] for managing one-off delayed jobs.
    pub fn delayed(&self) -> DelayedJobsClient {
        DelayedJobsClient {
            http: Arc::clone(&self.http),
        }
    }
}

/// Builder for a standalone [`Clockwork`] client.
///
/// # Example
///
/// ```no_run
/// use nautilus_rs::Clockwork;
///
/// let clockwork = Clockwork::builder()
///     .api_key("vrn_clockwork_live_sk_…")
///     .timeout_secs(15)
///     .build()
///     .expect("invalid configuration");
/// ```
#[derive(Default)]
pub struct ClockworkBuilder {
    api_key: Option<String>,
    base_url: Option<String>,
    timeout_secs: Option<u64>,
}

impl ClockworkBuilder {
    /// Set the Clockwork API key (**required**).
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Override the API base URL (default: `https://api.vernesoft.com`).
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Set the HTTP request timeout in seconds (default: `30`).
    pub fn timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

    /// Consume the builder and return a configured [`Clockwork`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] if the API key was not set.
    pub fn build(self) -> Result<Clockwork, Error> {
        let key = self
            .api_key
            .ok_or_else(|| Error::Config("clockwork API key is required".into()))?;
        let http = HttpClient::new(key, self.base_url, self.timeout_secs)?;
        Ok(Clockwork {
            http: Arc::new(http),
        })
    }
}

/// Access to the `/v1/clockwork/jobs` endpoints.
///
/// Obtain via [`Clockwork::jobs`].
pub struct CronJobsClient {
    http: Arc<HttpClient>,
}

impl CronJobsClient {
    /// List all cron jobs.
    ///
    /// Maps to `GET /v1/clockwork/jobs`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nautilus_rs::Clockwork;
    ///
    /// # async fn run() -> Result<(), nautilus_rs::Error> {
    /// let clockwork = Clockwork::new("vrn_clockwork_live_sk_…");
    /// for job in clockwork.jobs().list().await? {
    ///     println!("{} — {}", job.name, job.schedule);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list(&self) -> Result<Vec<CronJob>, Error> {
        self.http.get("/v1/clockwork/jobs").await
    }

    /// Create a new cron job.
    ///
    /// Maps to `POST /v1/clockwork/jobs`.
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
    /// println!("created job {}", job.id);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create(&self, params: CreateCronJobParams) -> Result<CronJob, Error> {
        self.http.post("/v1/clockwork/jobs", &params, false).await
    }

    /// Partially update an existing cron job.
    ///
    /// Only the fields set on `params` are sent; the rest are left unchanged.
    /// Maps to `PATCH /v1/clockwork/jobs/{id}`.
    pub async fn update(
        &self,
        job_id: &str,
        params: UpdateCronJobParams,
    ) -> Result<CronJob, Error> {
        self.http
            .patch(&format!("/v1/clockwork/jobs/{job_id}"), &params)
            .await
    }

    /// Permanently delete a cron job.
    ///
    /// Maps to `DELETE /v1/clockwork/jobs/{id}`.
    pub async fn delete(&self, job_id: &str) -> Result<(), Error> {
        self.http
            .delete(&format!("/v1/clockwork/jobs/{job_id}"))
            .await
    }

    /// List the execution history for a cron job.
    ///
    /// Maps to `GET /v1/clockwork/jobs/{id}/executions`.
    pub async fn executions(&self, job_id: &str) -> Result<Vec<Execution>, Error> {
        self.http
            .get(&format!("/v1/clockwork/jobs/{job_id}/executions"))
            .await
    }
}

/// Access to the `/v1/clockwork/delayed` endpoints.
///
/// Obtain via [`Clockwork::delayed`].
pub struct DelayedJobsClient {
    http: Arc<HttpClient>,
}

impl DelayedJobsClient {
    /// List all delayed jobs.
    ///
    /// Maps to `GET /v1/clockwork/delayed`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nautilus_rs::Clockwork;
    ///
    /// # async fn run() -> Result<(), nautilus_rs::Error> {
    /// let clockwork = Clockwork::new("vrn_clockwork_live_sk_…");
    /// for job in clockwork.delayed().list().await? {
    ///     println!("{} runs at {}", job.name, job.run_at);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list(&self) -> Result<Vec<DelayedJob>, Error> {
        self.http.get("/v1/clockwork/delayed").await
    }

    /// Schedule a new one-off delayed job.
    ///
    /// Maps to `POST /v1/clockwork/delayed`.
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
    /// println!("scheduled job {}", job.id);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create(&self, params: CreateDelayedJobParams) -> Result<DelayedJob, Error> {
        self.http
            .post("/v1/clockwork/delayed", &params, false)
            .await
    }

    /// Cancel a pending delayed job.
    ///
    /// Maps to `DELETE /v1/clockwork/delayed/{id}`.
    pub async fn cancel(&self, job_id: &str) -> Result<(), Error> {
        self.http
            .delete(&format!("/v1/clockwork/delayed/{job_id}"))
            .await
    }

    /// List the execution history for a delayed job.
    ///
    /// Maps to `GET /v1/clockwork/delayed/{id}/executions`.
    pub async fn executions(&self, job_id: &str) -> Result<Vec<Execution>, Error> {
        self.http
            .get(&format!("/v1/clockwork/delayed/{job_id}/executions"))
            .await
    }
}
