use std::sync::Arc;

use reqwest::Method;
use serde_json::{json, Value};

use super::types::{
    LifecycleAction, LifecycleRule, LifecycleRuleKind, LifecycleRules, ListLifecycleActionsParams,
    RulePreview, SetRuleParams,
};
use crate::{
    error::Error,
    http::{Call, HttpClient},
    types::Paginated,
};

fn rule_path(kind: LifecycleRuleKind) -> String {
    format!("/v1/gate/lifecycle/rules/{}", kind.as_str())
}

/// Automatic deletion of unconfirmed and inactive accounts, and removal of expired
/// roles — four rules per project, all off until you switch them on. Only
/// `delete_unverified` is in the Free plan; switching on another is `403 plan_required`.
///
/// Obtain via [`Gate::lifecycle`](super::Gate::lifecycle).
pub struct LifecycleClient {
    pub(super) http: Arc<HttpClient>,
}

impl LifecycleClient {
    /// All four rules of the key's project, set up or not, and the plan.
    ///
    /// Maps to `GET /v1/gate/lifecycle/rules`.
    pub async fn list_rules(&self) -> Result<LifecycleRules, Error> {
        self.http.get("/v1/gate/lifecycle/rules").await
    }

    /// Create, change, or switch a rule on or off. Leave `params` out to keep the saved
    /// settings (a first write needs them, except `expire_roles`). Switching off or
    /// changing the settings cancels what the rule had scheduled.
    ///
    /// Maps to `PUT /v1/gate/lifecycle/rules/{kind}`. `400 invalid_payload` for settings
    /// out of bounds (see [`LifecycleRuleKind`]); `403 plan_required` when the rule is
    /// not in your plan (only when enabling).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nautilus_rs::{Gate, LifecycleRuleKind, SetRuleParams};
    /// use serde_json::json;
    ///
    /// # async fn run() -> Result<(), nautilus_rs::Error> {
    /// let gate = Gate::new("vrn_9f8a7c1b2d3e4f50");
    /// let rule = gate.lifecycle().set_rule(LifecycleRuleKind::DeleteUnverified, SetRuleParams {
    ///     enabled: true,
    ///     params: Some(json!({ "delete_after_days": 14, "warn_before_days": 3 })),
    /// }).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn set_rule(
        &self,
        kind: LifecycleRuleKind,
        params: SetRuleParams,
    ) -> Result<LifecycleRule, Error> {
        self.http.put(&rule_path(kind), &params).await
    }

    /// Remove a rule and cancel what it scheduled. Succeeds (`204`) also when there was
    /// none.
    ///
    /// Maps to `DELETE /v1/gate/lifecycle/rules/{kind}`.
    pub async fn delete_rule(&self, kind: LifecycleRuleKind) -> Result<(), Error> {
        self.http.delete(&rule_path(kind)).await
    }

    /// How many people a rule with these settings would affect right now. Writes nothing;
    /// works whether or not the rule is on or in your plan (`available: false`).
    /// `expire_roles` takes `json!({})`.
    ///
    /// Maps to `POST /v1/gate/lifecycle/rules/{kind}/preview`; the body is
    /// `{"params": …}`.
    pub async fn preview_rule(
        &self,
        kind: LifecycleRuleKind,
        params: Value,
    ) -> Result<RulePreview, Error> {
        self.http
            .post(
                &format!("{}/preview", rule_path(kind)),
                &json!({ "params": params }),
            )
            .await
    }

    /// What the rules have scheduled and done, oldest first, cursor-paged.
    ///
    /// Maps to `GET /v1/gate/lifecycle/actions`.
    pub async fn list_actions(
        &self,
        params: ListLifecycleActionsParams,
    ) -> Result<Paginated<LifecycleAction>, Error> {
        let call = Call::new(Method::GET, "/v1/gate/lifecycle/actions")
            .query("limit", params.limit)
            .query("cursor", params.cursor)
            .query("status", params.status)
            .query("kind", params.kind.map(|k| k.as_str()))
            .query("identity_id", params.identity_id);
        self.http.send(call).await
    }
}
