use std::sync::Arc;

use reqwest::Method;

use super::types::{AuditLogEntry, ListAuditLogParams};
use crate::{
    error::Error,
    http::{Call, HttpClient},
    types::Paginated,
};

/// The account's audit log, read-only: who did what, to what, when and from where. The
/// key reads its own project's entries and the account-wide ones.
///
/// **Any key with the `gate` scope reads the whole log, including the IP addresses and
/// user agents of your team members.** Keep `gate` keys on your backend.
///
/// Obtain via [`Gate::audit_log`](super::Gate::audit_log).
pub struct AuditLogClient {
    pub(super) http: Arc<HttpClient>,
}

impl AuditLogClient {
    /// One page of entries, newest first. Filters combine; keep the same filters while
    /// paging with `cursor`.
    ///
    /// Maps to `GET /v1/gate/audit-log`. `400 invalid_payload` for an unknown `action`, a
    /// non-UUID `actor_id`, a bad time, `from >= to`, or a bad cursor.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nautilus_rs::{Gate, ListAuditLogParams};
    ///
    /// # async fn run() -> Result<(), nautilus_rs::Error> {
    /// let gate = Gate::new("vrn_9f8a7c1b2d3e4f50");
    /// let page = gate.audit_log().list(ListAuditLogParams {
    ///     action: Some("identity.login".into()),
    ///     limit: Some(50),
    ///     ..Default::default()
    /// }).await?;
    /// for entry in &page.data {
    ///     println!("{} {} {:?}", entry.occurred_at, entry.action, entry.target);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list(
        &self,
        params: ListAuditLogParams,
    ) -> Result<Paginated<AuditLogEntry>, Error> {
        let call = Call::new(Method::GET, "/v1/gate/audit-log")
            .query("limit", params.limit)
            .query("cursor", params.cursor)
            .query("action", params.action)
            .query("actor_id", params.actor_id)
            .query("actor_type", params.actor_type)
            .query("target_id", params.target_id)
            .query("from", params.from)
            .query("to", params.to);
        self.http.send(call).await
    }
}
