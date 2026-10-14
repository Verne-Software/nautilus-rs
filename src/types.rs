/// A page of results from a list endpoint.
///
/// Use `next_cursor` with the corresponding `cursor` parameter to fetch the
/// following page when `has_more` is `true`.
///
/// # Example
///
/// ```no_run
/// use nautilus_rs::{Relay, ListMessagesParams, Paginated, Message};
///
/// # async fn run() -> Result<(), nautilus_rs::Error> {
/// let relay = Relay::new("vrn_9f8a7c1b2d3e4f50");
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

/// Cursor-paging parameters shared by the Gate list endpoints (organizations, roles,
/// members, a user's organizations).
///
/// Loop on [`Paginated::has_more`], passing the previous page's
/// [`next_cursor`](Paginated::next_cursor) as `cursor`.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ListParams {
    /// Page size, 1–100 (default 20). The server clamps an out-of-range value instead of
    /// refusing it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// The previous page's `next_cursor`. Opaque; do not build one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}
