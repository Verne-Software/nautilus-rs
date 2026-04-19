/// A page of results from a list endpoint.
///
/// Use `next_cursor` with the corresponding `cursor` parameter to fetch the
/// following page when `has_more` is `true`.
///
/// # Example
///
/// ```no_run
/// use nautilus::{Relay, ListMessagesParams, Paginated, Message};
///
/// # async fn run() -> Result<(), nautilus::Error> {
/// let relay = Relay::new("vrn_relay_live_sk_…");
/// let mut cursor: Option<String> = None;
///
/// loop {
///     let page: Paginated<Message> = relay.messages().list(ListMessagesParams {
///         cursor: cursor.clone(),
///         limit: Some(50),
///         ..Default::default()
///     }).await?;
///
///     for msg in &page.data {
///         println!("{}: {}", msg.id, msg.event_type);
///     }
///
///     if !page.has_more { break; }
///     cursor = page.next_cursor;
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Paginated<T> {
    /// The items on this page.
    pub data: Vec<T>,
    /// Whether there are more pages after this one.
    pub has_more: bool,
    /// Opaque cursor to pass as `cursor` in the next request. `None` when
    /// `has_more` is `false`.
    pub next_cursor: Option<String>,
}
