pub mod types;

use std::sync::Arc;

use crate::{error::Error, http::HttpClient, types::Paginated};
use types::{ListMessagesParams, Message, SendMessageParams};

/// Relay service client — Webhooks-as-a-Service.
///
/// Delivers JSON events to HTTP endpoints that have subscribed through the
/// Verne dashboard or the Relay API.
///
/// Obtain a `Relay` instance either as part of the unified [`Verne`] client or
/// standalone:
///
/// ```no_run
/// // Standalone
/// use nautilus_rs::Relay;
/// let relay = Relay::new("vrn_relay_live_sk_…");
///
/// // Via unified client
/// use nautilus_rs::Verne;
/// # fn run() -> Result<(), nautilus_rs::Error> {
/// let verne = Verne::builder().relay("vrn_relay_live_sk_…").build()?;
/// let relay = verne.relay()?;
/// # Ok(())
/// # }
/// ```
///
/// [`Verne`]: crate::Verne
pub struct Relay {
    http: Arc<HttpClient>,
}

impl std::fmt::Debug for Relay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Relay").finish_non_exhaustive()
    }
}

impl Relay {
    /// Create a `Relay` client with default settings.
    ///
    /// Panics if the API key is empty or the HTTP client cannot be
    /// initialised. Use [`Relay::builder`] for fallible construction.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::builder()
            .api_key(api_key)
            .build()
            .expect("failed to build Relay client")
    }

    /// Return a [`RelayBuilder`] for fine-grained configuration.
    pub fn builder() -> RelayBuilder {
        RelayBuilder::default()
    }

    pub(crate) fn from_http(http: Arc<HttpClient>) -> Self {
        Self { http }
    }

    /// Return a [`MessagesClient`] for sending and listing webhook messages.
    pub fn messages(&self) -> MessagesClient {
        MessagesClient {
            http: Arc::clone(&self.http),
        }
    }
}

/// Builder for a standalone [`Relay`] client.
///
/// # Example
///
/// ```no_run
/// use nautilus_rs::Relay;
///
/// let relay = Relay::builder()
///     .api_key("vrn_relay_live_sk_…")
///     .timeout_secs(15)
///     .build()
///     .expect("invalid configuration");
/// ```
#[derive(Default)]
pub struct RelayBuilder {
    api_key: Option<String>,
    base_url: Option<String>,
    timeout_secs: Option<u64>,
}

impl RelayBuilder {
    /// Set the Relay API key (**required**).
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

    /// Consume the builder and return a configured [`Relay`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] if the API key was not set.
    pub fn build(self) -> Result<Relay, Error> {
        let key = self
            .api_key
            .ok_or_else(|| Error::Config("relay API key is required".into()))?;
        let http = HttpClient::new(key, self.base_url, self.timeout_secs)?;
        Ok(Relay {
            http: Arc::new(http),
        })
    }
}

/// Access to the `/v1/relay/messages` endpoints.
///
/// Obtain via [`Relay::messages`].
pub struct MessagesClient {
    http: Arc<HttpClient>,
}

impl MessagesClient {
    /// Send an event to all subscribed endpoints.
    ///
    /// Maps to `POST /v1/relay/messages`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nautilus_rs::{Relay, SendMessageParams};
    /// use serde_json::json;
    ///
    /// # async fn run() -> Result<(), nautilus_rs::Error> {
    /// let relay = Relay::new("vrn_relay_live_sk_…");
    /// let msg = relay.messages().send(SendMessageParams {
    ///     event_type: "user.signed_up".into(),
    ///     payload: json!({ "user_id": "usr_abc" }),
    ///     ..Default::default()
    /// }).await?;
    /// println!("message id: {}", msg.id);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send(&self, params: SendMessageParams) -> Result<Message, Error> {
        self.http.post("/v1/relay/messages", &params, false).await
    }

    /// Retrieve a paginated list of past messages.
    ///
    /// Maps to `GET /v1/relay/messages`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nautilus_rs::{Relay, ListMessagesParams};
    ///
    /// # async fn run() -> Result<(), nautilus_rs::Error> {
    /// let relay = Relay::new("vrn_relay_live_sk_…");
    /// let page = relay.messages().list(ListMessagesParams {
    ///     limit: Some(20),
    ///     event_type: Some("order.placed".into()),
    ///     ..Default::default()
    /// }).await?;
    ///
    /// for msg in &page.data {
    ///     println!("{} — {}", msg.timestamp, msg.event_type);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list(&self, params: ListMessagesParams) -> Result<Paginated<Message>, Error> {
        let mut query = vec![];
        if let Some(limit) = params.limit {
            query.push(format!("limit={limit}"));
        }
        if let Some(cursor) = &params.cursor {
            query.push(format!("cursor={cursor}"));
        }
        if let Some(event_type) = &params.event_type {
            query.push(format!("event_type={event_type}"));
        }

        let path = if query.is_empty() {
            "/v1/relay/messages".to_string()
        } else {
            format!("/v1/relay/messages?{}", query.join("&"))
        };

        self.http.get(&path).await
    }
}
