use std::sync::Arc;

use crate::{
    error::Error,
    http::HttpClient,
    resources::{gate::Gate, relay::Relay},
};

/// Builder for the unified [`Verne`] client.
///
/// Obtain one via [`Verne::builder()`]. All setter methods are chainable. At
/// least one of [`relay`](VerneBuilder::relay) or [`gate`](VerneBuilder::gate)
/// must be set; calling [`build`](VerneBuilder::build) with neither is valid
/// but the resulting client will return [`Error::Config`] for every service
/// accessor.
///
/// # Example
///
/// ```no_run
/// use nautilus_rs::Verne;
///
/// let verne = Verne::builder()
///     .relay("vrn_relay_live_sk_…")
///     .gate("vrn_gate_live_sk_…")
///     .timeout_secs(10)
///     .build()
///     .expect("invalid configuration");
/// ```
#[derive(Default)]
pub struct VerneBuilder {
    relay_key: Option<String>,
    gate_key: Option<String>,
    base_url: Option<String>,
    timeout_secs: Option<u64>,
}

impl VerneBuilder {
    /// Set the Relay service API key (`vrn_relay_<env>_sk_…`).
    pub fn relay(mut self, key: impl Into<String>) -> Self {
        self.relay_key = Some(key.into());
        self
    }

    /// Set the Gate service API key (`vrn_gate_<env>_sk_…`).
    pub fn gate(mut self, key: impl Into<String>) -> Self {
        self.gate_key = Some(key.into());
        self
    }

    /// Override the API base URL (default: `https://api.vernesoft.com`).
    ///
    /// Useful for pointing at a staging environment or a local mock server
    /// during tests.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Set the HTTP request timeout in seconds (default: `30`).
    pub fn timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

    /// Consume the builder and return a [`Verne`] client.
    ///
    /// Returns [`Error::Config`] if the underlying HTTP client cannot be
    /// initialised (e.g. invalid TLS configuration).
    pub fn build(self) -> Result<Verne, Error> {
        let relay = self
            .relay_key
            .map(|key| {
                let http = HttpClient::new(&key, self.base_url.clone(), self.timeout_secs)?;
                Ok::<_, Error>(Arc::new(Relay::from_http(Arc::new(http))))
            })
            .transpose()?;

        let gate = self
            .gate_key
            .map(|key| {
                let http = HttpClient::new(&key, self.base_url.clone(), self.timeout_secs)?;
                Ok::<_, Error>(Arc::new(Gate::from_http(Arc::new(http), key)))
            })
            .transpose()?;

        Ok(Verne { relay, gate })
    }
}

/// Unified entry point for the Verne Nautilus platform SDK.
///
/// Holds optional, cheaply-cloneable handles to the [`Relay`] and [`Gate`]
/// services. Construct it with [`Verne::builder()`].
///
/// # Example
///
/// ```no_run
/// use nautilus_rs::Verne;
///
/// # async fn run() -> Result<(), nautilus_rs::Error> {
/// let verne = Verne::builder()
///     .relay("vrn_relay_live_sk_…")
///     .gate("vrn_gate_live_sk_…")
///     .build()?;
///
/// let relay = verne.relay()?;
/// let gate  = verne.gate()?;
/// # Ok(())
/// # }
/// ```
pub struct Verne {
    relay: Option<Arc<Relay>>,
    gate: Option<Arc<Gate>>,
}

impl Verne {
    /// Create a new [`VerneBuilder`].
    pub fn builder() -> VerneBuilder {
        VerneBuilder::default()
    }

    /// Return a reference to the [`Relay`] service.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] if no Relay API key was provided at build
    /// time.
    pub fn relay(&self) -> Result<&Relay, Error> {
        self.relay
            .as_deref()
            .ok_or_else(|| Error::Config("no relay API key configured".into()))
    }

    /// Return a reference to the [`Gate`] service.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] if no Gate API key was provided at build
    /// time.
    pub fn gate(&self) -> Result<&Gate, Error> {
        self.gate
            .as_deref()
            .ok_or_else(|| Error::Config("no gate API key configured".into()))
    }
}