/// A webhook message returned by the Relay API.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Message {
    /// Unique message identifier.
    pub id: String,
    /// The event type that was dispatched (e.g. `"order.placed"`).
    pub event_type: String,
    /// Always `"accepted"`.
    ///
    /// It records that Relay took the event, not what each subscriber endpoint
    /// did with it afterwards — per-endpoint delivery state lives in the
    /// Console under Dashboard → Relay. This used to be documented as
    /// `"pending"` / `"delivered"` / `"failed"`, which the API has never
    /// returned. Left as `String` rather than narrowed to an enum so that a
    /// future value cannot turn into a deserialisation failure in a client that
    /// only wanted the id.
    pub status: String,
    /// ISO 8601 timestamp of when the message was created.
    pub timestamp: String,
}

/// Parameters for [`MessagesClient::send`](super::MessagesClient::send).
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
    /// Optional idempotency key, deduplicating within a 24-hour window.
    ///
    /// Reusing the same key returns the *originally* accepted message — same
    /// `id`, same `timestamp` — rather than creating a second event or
    /// failing. So retrying a request whose response you never saw needs no
    /// special handling: there is no duplicate to tell apart from a success.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    /// Restrict delivery to the named channels. `None` delivers to all channels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<Vec<String>>,
}

/// Parameters for [`MessagesClient::list`](super::MessagesClient::list).
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ListMessagesParams {
    /// Maximum number of messages to return. The server default (20) applies
    /// when `None`, and anything above 100 is clamped to 100 rather than
    /// rejected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Pagination cursor from the previous page's
    /// [`next_cursor`](crate::Paginated::next_cursor).
    ///
    /// That field is `None` on the last page, so paginate until
    /// [`has_more`](crate::Paginated::has_more) is `false` rather than until
    /// `data` comes back empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// Filter results to a specific event type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_type: Option<String>,
}
