use std::sync::Arc;

use reqwest::Method;

use super::types::{Session, SessionCheck};
use crate::{
    error::Error,
    http::{urlencode as seg, Call, HttpClient},
};

/// End users' sessions: list, revoke, and check that one is good enough to act on.
///
/// Obtain via [`Gate::sessions`](super::Gate::sessions).
pub struct SessionsClient {
    pub(super) http: Arc<HttpClient>,
}

impl SessionsClient {
    /// The active sessions of one of your end users (a bare array, not paged).
    ///
    /// Maps to `GET /v1/gate/identities/{id}/sessions`. Another account's user is
    /// `403 forbidden`.
    pub async fn list(&self, identity_id: &str) -> Result<Vec<Session>, Error> {
        self.http
            .get(&format!(
                "/v1/gate/identities/{}/sessions",
                seg(identity_id)
            ))
            .await
    }

    /// Revoke every session of an end user — "log out everywhere". Signed tokens already
    /// issued keep verifying until their own `exp` (at most 15 minutes).
    ///
    /// Maps to `DELETE /v1/gate/identities/{id}/sessions`. Note: the API answers `404`
    /// (with the identity service's own body, a numeric `code`) when the user has no
    /// session at all; it surfaces here as an [`Error::Api`] with `status == 404`.
    pub async fn revoke_all(&self, identity_id: &str) -> Result<(), Error> {
        self.http
            .delete(&format!(
                "/v1/gate/identities/{}/sessions",
                seg(identity_id)
            ))
            .await
    }

    /// Revoke one session by its id. Revoking an already revoked session of your user is
    /// a success too; an id that is not one of your users' sessions (unknown, foreign,
    /// not a UUID) is `404 not_found`.
    ///
    /// Maps to `DELETE /v1/gate/sessions/{session_id}`.
    pub async fn revoke(&self, session_id: &str) -> Result<(), Error> {
        self.http
            .delete(&format!("/v1/gate/sessions/{}", seg(session_id)))
            .await
    }

    /// Whether an end user's session meets what your tenant requires — the one call to
    /// gate your own routes on. Gate on [`mfa_satisfied`](SessionCheck::mfa_satisfied).
    /// Also lists the user's organizations in the key's project, read now.
    ///
    /// Maps to `GET /v1/gate/auth/session` with `X-Session-Token`. Errors:
    /// `401 unauthorized` for a dead, foreign or unknown session; `403 mfa_required` when
    /// a second factor is owed, with `details["required_aal"] == "aal2"` and
    /// `details["current_aal"]` — start a step-up flow with
    /// [`Gate::create_login_flow_with`](super::Gate::create_login_flow_with) (`aal:
    /// Some("aal2")`, the `session_token`); `502` is worth a retry.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nautilus_rs::{Error, Gate};
    ///
    /// # async fn run(session_token: &str) -> Result<(), Error> {
    /// let gate = Gate::new("vrn_9f8a7c1b2d3e4f50");
    /// match gate.sessions().check(session_token).await {
    ///     Ok(check) if check.mfa_satisfied => println!("welcome {:?}", check.identity_id),
    ///     Ok(_) => println!("a second factor is owed"),
    ///     Err(Error::Api(e)) if e.status == 403 && e.code == "mfa_required" => {
    ///         // send the user through a step-up flow
    ///     }
    ///     Err(Error::Api(e)) if e.status == 401 => println!("sign in again"),
    ///     Err(e) => return Err(e),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn check(&self, session_token: &str) -> Result<SessionCheck, Error> {
        self.http
            .send(Call::new(Method::GET, "/v1/gate/auth/session").session_token(session_token))
            .await
    }
}
