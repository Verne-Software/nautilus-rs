use std::sync::Arc;

use crate::{
    error::Error,
    http::HttpClient,
    resources::{clockwork::Clockwork, gate::Gate, passepartout::Passepartout, relay::Relay},
};

/// Builder for the unified [`Verne`] client.
///
/// Obtain one via [`Verne::builder()`]. All setter methods are chainable.
///
/// A key is `vrn_…` and carries a set of scopes (`relay`, `gate`, `clockwork`,
/// `passepartout`) chosen when it is created on Dashboard → Keys. **Pass one key with
/// [`api_key`](Self::api_key)** and use every service its scopes allow; a call to a
/// service the key has no scope for is refused by the API with `403 forbidden`. The SDK
/// does not check scopes and does not parse the key.
///
/// The per-service setters ([`relay`](Self::relay), [`gate`](Self::gate),
/// [`passepartout`](Self::passepartout), [`clockwork`](Self::clockwork)) remain, and
/// **override** `api_key` for that one service — for different keys per service, or for
/// older `vrn_<engine>_…` keys, which still work and open that one engine.
///
/// There is no test/live environment and no sandbox: use a separate key per environment.
///
/// [`build`](VerneBuilder::build) with no key at all is valid, but the resulting client
/// will return [`Error::Config`] for every service accessor.
///
/// # Example
///
/// ```no_run
/// use nautilus_rs::Verne;
///
/// let verne = Verne::builder()
///     .api_key("vrn_9f8a7c1b2d3e4f50")
///     .timeout_secs(10)
///     .build()
///     .expect("invalid configuration");
/// ```
#[derive(Default)]
pub struct VerneBuilder {
    api_key: Option<String>,
    relay_key: Option<String>,
    gate_key: Option<String>,
    passepartout_key: Option<String>,
    clockwork_key: Option<String>,
    base_url: Option<String>,
    timeout_secs: Option<u64>,
}

impl VerneBuilder {
    /// Set the one API key (`vrn_…`) used for every service that has no key of its own.
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Set a key for Relay only, overriding [`api_key`](Self::api_key).
    pub fn relay(mut self, key: impl Into<String>) -> Self {
        self.relay_key = Some(key.into());
        self
    }

    /// Set a key for Gate only, overriding [`api_key`](Self::api_key).
    pub fn gate(mut self, key: impl Into<String>) -> Self {
        self.gate_key = Some(key.into());
        self
    }

    /// Set a key for Passepartout only, overriding [`api_key`](Self::api_key).
    pub fn passepartout(mut self, key: impl Into<String>) -> Self {
        self.passepartout_key = Some(key.into());
        self
    }

    /// Set a key for Clockwork only, overriding [`api_key`](Self::api_key).
    pub fn clockwork(mut self, key: impl Into<String>) -> Self {
        self.clockwork_key = Some(key.into());
        self
    }

    /// Override the API base URL (default: `https://api.vernesoft.com`).
    ///
    /// Useful for pointing at a local stack or a mock server during tests.
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
        let shared = self.api_key.as_ref();
        let http = |own: Option<&String>| {
            own.or(shared)
                .map(|key| {
                    HttpClient::new(key, self.base_url.clone(), self.timeout_secs).map(Arc::new)
                })
                .transpose()
        };

        let relay = http(self.relay_key.as_ref())?.map(|http| Arc::new(Relay::from_http(http)));
        let gate = http(self.gate_key.as_ref())?.map(|http| Arc::new(Gate::from_http(http)));
        let passepartout = http(self.passepartout_key.as_ref())?
            .map(|http| Arc::new(Passepartout::from_http(http)));
        let clockwork =
            http(self.clockwork_key.as_ref())?.map(|http| Arc::new(Clockwork::from_http(http)));

        Ok(Verne {
            relay,
            gate,
            passepartout,
            clockwork,
        })
    }
}

/// Unified entry point for the Verne Nautilus platform SDK.
///
/// Holds optional, cheaply-cloneable handles to the [`Relay`], [`Gate`],
/// [`Passepartout`] and [`Clockwork`] services. Construct it with [`Verne::builder()`].
///
/// # Example
///
/// ```no_run
/// use nautilus_rs::Verne;
///
/// # async fn run() -> Result<(), nautilus_rs::Error> {
/// let verne = Verne::builder().api_key("vrn_9f8a7c1b2d3e4f50").build()?;
///
/// let relay = verne.relay()?;
/// let gate  = verne.gate()?;
/// # Ok(())
/// # }
/// ```
pub struct Verne {
    relay: Option<Arc<Relay>>,
    gate: Option<Arc<Gate>>,
    passepartout: Option<Arc<Passepartout>>,
    clockwork: Option<Arc<Clockwork>>,
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
    /// Returns [`Error::Config`] if neither `api_key` nor a Relay key was provided at
    /// build time.
    pub fn relay(&self) -> Result<&Relay, Error> {
        self.relay.as_deref().ok_or_else(|| {
            Error::Config(
                "no relay API key configured: set `api_key` (one key for every service) or `relay`"
                    .into(),
            )
        })
    }

    /// Return a reference to the [`Gate`] service.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] if neither `api_key` nor a Gate key was provided at
    /// build time.
    pub fn gate(&self) -> Result<&Gate, Error> {
        self.gate.as_deref().ok_or_else(|| {
            Error::Config(
                "no gate API key configured: set `api_key` (one key for every service) or `gate`"
                    .into(),
            )
        })
    }

    /// Return a reference to the [`Passepartout`] service.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] if neither `api_key` nor a Passepartout key was provided at
    /// build time.
    pub fn passepartout(&self) -> Result<&Passepartout, Error> {
        self.passepartout
            .as_deref()
            .ok_or_else(|| Error::Config(
                "no passepartout API key configured: set `api_key` (one key for every service) or `passepartout`"
                    .into(),
            ))
    }

    /// Return a reference to the [`Clockwork`] service.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] if neither `api_key` nor a Clockwork key was provided at
    /// build time.
    pub fn clockwork(&self) -> Result<&Clockwork, Error> {
        self.clockwork
            .as_deref()
            .ok_or_else(|| Error::Config(
                "no clockwork API key configured: set `api_key` (one key for every service) or `clockwork`"
                    .into(),
            ))
    }
}
