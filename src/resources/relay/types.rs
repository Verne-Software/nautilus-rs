/// A webhook message returned by the Relay API.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Message {
    /// Unique message identifier.
    pub id: String,
    /// The event type that was dispatched (e.g. `"order.placed"`).
    pub event_type: String,
    /// Delivery status (`"pending"`, `"delivered"`, `"failed"`, …).
    pub status: String,
    /// ISO 8601 timestamp of when the message was created.
    pub timestamp: String,
}

/// Parameters for [`MessagesClient::send`](super::MessagesClient::send).
///
/// # Example
///
/// ```no_run
/// use nautilus::{Relay, SendMessageParams};
/// use serde_json::json;
///
/// # async fn run() -> Result<(), nautilus::Error> {
/// let relay = Relay::new("vrn_relay_live_sk_…");
/// let msg = relay.messages().send(SendMessageParams {
///     event_type: "order.placed".into(),
///     payload: json!({ "order_id": "ord_123", "total": 49.99 }),
///     idempotency_key: Some("ord_123-placed".into()),
///     channels: Some(vec!["primary".into()]),
/// }).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct SendMessageParams {
    /// The event type to dispatch (e.g. `"order.placed"`).
    pub event_type: String,
    /// Arbitrary JSON payload delivered to all subscribed endpoints.
    pub payload: serde_json::Value,
    /// Optional idempotency key — reusing the same key within the deduplication
    /// window returns the original message instead of creating a new one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    /// Restrict delivery to the named channels. `None` delivers to all channels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<Vec<String>>,
}

/// Parameters for [`MessagesClient::list`](super::MessagesClient::list).
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ListMessagesParams {
    /// Maximum number of messages to return (server default applies when
    /// `None`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Pagination cursor from the previous page's
    /// [`next_cursor`](crate::Paginated::next_cursor).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// Filter results to a specific event type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_type: Option<String>,
}
