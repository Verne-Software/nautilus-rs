mod common;

use common::{gate, JSON, KEY, ORG, USER};
use mockito::Matcher;
use nautilus_rs::{
    Error, LifecycleRuleKind, ListAuditLogParams, ListLifecycleActionsParams, LoginFlowParams,
    RegistrationFlowParams, SecuritySettings, SetRuleParams,
};
use serde_json::json;

const SESSION_TOKEN: &str = "ory_st_k3mZ9";

fn session_json(id: &str) -> String {
    format!(
        r#"{{"id":"{id}","active":true,"authenticated_at":"2026-10-14T10:00:00Z","expires_at":"2026-10-15T10:00:00Z","issued_at":"2026-10-14T10:00:00Z","authenticator_assurance_level":"aal1","authentication_methods":[{{"method":"password","completed_at":"2026-10-14T10:00:00Z"}}],"devices":[{{"id":"d1","ip_address":"203.0.113.9","user_agent":"curl","location":"Paris"}}],"identity":{{"id":"{USER}"}}}}"#
    )
}

// --- identities ---------------------------------------------------------------------

#[tokio::test]
async fn identity_traits_are_email_and_locale_and_the_rest_stays_reachable() {
    let (mut server, gate) = gate().await;
    server
        .mock("GET", "/v1/gate/identities/idn_1")
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(format!(
            r#"{{"id":"idn_1","schema_id":"user","state":"active","traits":{{"email":"ada@example.com","locale":"fr"}},
            "created_at":"2026-10-14T10:00:00Z","updated_at":"2026-10-14T10:00:00Z","state_changed_at":"2026-10-14T10:00:00Z",
            "metadata_public":{{"plan":"pro"}},
            "verifiable_addresses":[{{"value":"ada@example.com","verified":true,"via":"email","status":"completed"}}],
            "recovery_addresses":[{{"value":"ada@example.com","via":"email"}}],
            "credentials":{{"password":{{"type":"password"}}}},"organization_id":"{ORG}"}}"#
        ))
        .create_async()
        .await;

    let identity = gate.identities().get("idn_1").await.unwrap();

    assert_eq!(identity.traits.email, "ada@example.com");
    assert_eq!(identity.traits.locale.as_deref(), Some("fr"));
    assert_eq!(identity.metadata_public.unwrap()["plan"], "pro");
    assert!(identity.verifiable_addresses[0].verified);
    assert_eq!(identity.recovery_addresses[0].via, "email");
    assert!(identity.created_at.is_some());
    // Everything not typed is still there.
    assert!(identity.extra.contains_key("credentials"));
    assert_eq!(identity.extra["organization_id"], ORG);
}

#[tokio::test]
async fn create_identity_sends_email_and_locale_and_no_custom_data() {
    let (mut server, gate) = gate().await;
    let mock = server
        .mock("POST", "/v1/gate/identities")
        .match_body(Matcher::Json(json!({
            "schema_id": "user",
            "traits": {"email": "ada@example.com", "locale": "fr"},
        })))
        .with_status(201)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"id":"idn_1","schema_id":"user","state":"active","traits":{"email":"ada@example.com","locale":"fr"}}"#)
        .create_async()
        .await;

    gate.identities()
        .create(nautilus_rs::CreateIdentityParams {
            schema_id: "user".into(),
            traits: nautilus_rs::IdentityTraitsInput {
                email: "ada@example.com".into(),
                locale: Some("fr".into()),
            },
            credentials: None,
            state: None,
            ..Default::default()
        })
        .await
        .unwrap();
    mock.assert_async().await;
}

#[tokio::test]
async fn get_metadata_unwraps_metadata_public() {
    let (mut server, gate) = gate().await;
    let mock = server
        .mock(
            "GET",
            format!("/v1/gate/identities/{USER}/metadata").as_str(),
        )
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"metadata_public":{"plan":"pro","seats":5}}"#)
        .create_async()
        .await;

    let metadata = gate.identities().get_metadata(USER).await.unwrap();

    assert_eq!(metadata["plan"], "pro");
    assert_eq!(metadata["seats"], 5);
    mock.assert_async().await;
}

#[tokio::test]
async fn empty_metadata_is_an_empty_map_even_when_the_server_says_null() {
    let (mut server, gate) = gate().await;
    server
        .mock(
            "GET",
            format!("/v1/gate/identities/{USER}/metadata").as_str(),
        )
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"metadata_public":null}"#)
        .create_async()
        .await;

    assert!(gate
        .identities()
        .get_metadata(USER)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn update_metadata_sends_the_patch_as_is_and_returns_the_merge() {
    let (mut server, gate) = gate().await;
    let mock = server
        .mock(
            "PATCH",
            format!("/v1/gate/identities/{USER}/metadata").as_str(),
        )
        .match_body(Matcher::Json(json!({"plan": "pro", "seats": null})))
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"metadata_public":{"plan":"pro"}}"#)
        .create_async()
        .await;

    let merged = gate
        .identities()
        .update_metadata(USER, json!({"plan": "pro", "seats": null}))
        .await
        .unwrap();

    assert_eq!(serde_json::Value::Object(merged), json!({"plan": "pro"}));
    mock.assert_async().await;
}

// --- tokens ------------------------------------------------------------------------

#[tokio::test]
async fn introspecting_an_inactive_token_is_just_active_false() {
    let (mut server, gate) = gate().await;
    server
        .mock("POST", "/v1/gate/tokens/introspect")
        .match_body(Matcher::Json(json!({"access_token": "gat_live_nope"})))
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"active":false}"#)
        .create_async()
        .await;

    let info = gate.tokens().introspect("gat_live_nope").await.unwrap();

    assert!(!info.active);
    assert_eq!(info.subject, None);
    assert_eq!(info.tenant_id, None);
    assert_eq!(info.scopes, None);
    assert_eq!(info.expires_at, None);
    assert_eq!(info.orgs, None);
}

#[tokio::test]
async fn introspection_carries_orgs() {
    let (mut server, gate) = gate().await;
    server
        .mock("POST", "/v1/gate/tokens/introspect")
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(format!(
            r#"{{"active":true,"subject":"{USER}","tenant_id":"t","scopes":[],"expires_at":"2026-10-14T11:00:00Z","orgs":[{{"id":"{ORG}","role":"admin","permissions":["invoices:write"]}}],"orgs_truncated":true}}"#
        ))
        .create_async()
        .await;

    let info = gate.tokens().introspect("gat_live_x").await.unwrap();

    let orgs = info.orgs.unwrap();
    assert_eq!(orgs[0].id, ORG);
    assert_eq!(orgs[0].role, "admin");
    assert_eq!(info.orgs_truncated, Some(true));
}

/// The session is the credential: no `Authorization` header, the token in `X-Session-Token`.
#[tokio::test]
async fn create_signed_sends_the_session_token_and_no_api_key() {
    let (mut server, gate) = gate().await;
    let mock = server
        .mock("GET", "/auth/sessions/whoami?tokenize_as=gate_jwt")
        .match_header("x-session-token", SESSION_TOKEN)
        .match_header("authorization", Matcher::Missing)
        .match_header("accept", "application/json")
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(format!(
            r#"{{{},"tokenized":"aaa.bbb.ccc"}}"#,
            // the session fields without the outer braces
            &session_json("sess_1")[1..session_json("sess_1").len() - 1]
        ))
        .create_async()
        .await;

    let signed = gate.tokens().create_signed(SESSION_TOKEN).await.unwrap();

    assert_eq!(signed.token, "aaa.bbb.ccc");
    assert_eq!(signed.session.id, "sess_1");
    // `tokenized` is split off, not left in the session.
    assert!(!signed.session.extra.contains_key("tokenized"));
    mock.assert_async().await;
}

/// The API answers `401 unauthorized` in the envelope now; a Kratos body with a numeric code
/// is still read, as a fallback.
#[tokio::test]
async fn create_signed_on_a_dead_session_is_a_401_with_the_numeric_code_read_as_unknown() {
    let (mut server, gate) = gate().await;
    server
        .mock("GET", "/auth/sessions/whoami?tokenize_as=gate_jwt")
        .with_status(401)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"error":{"code":401,"status":"Unauthorized","message":"No valid session cookie found."}}"#)
        .create_async()
        .await;

    let err = gate.tokens().create_signed("dead").await.unwrap_err();
    match err {
        Error::Api(e) => {
            assert_eq!(e.status, 401);
            assert_eq!(e.code, "unknown");
            assert_eq!(e.message, "No valid session cookie found.");
            assert_eq!(e.details["status"], "Unauthorized");
        }
        other => panic!("expected Error::Api, got {other:?}"),
    }
}

// --- sessions ----------------------------------------------------------------------

#[tokio::test]
async fn sessions_list_is_a_bare_array() {
    let (mut server, gate) = gate().await;
    let mock = server
        .mock(
            "GET",
            format!("/v1/gate/identities/{USER}/sessions").as_str(),
        )
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(format!("[{}]", session_json("sess_1")))
        .create_async()
        .await;

    let sessions = gate.sessions().list(USER).await.unwrap();

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "sess_1");
    assert_eq!(sessions[0].authentication_methods[0].method, "password");
    assert_eq!(
        sessions[0].devices[0].ip_address.as_deref(),
        Some("203.0.113.9")
    );
    assert_eq!(
        sessions[0].authenticator_assurance_level.as_deref(),
        Some("aal1")
    );
    mock.assert_async().await;
}

/// `revoke_all` is `204` also for a user with no session: `Ok(())`.
#[tokio::test]
async fn sessions_revoke_and_revoke_all() {
    let (mut server, gate) = gate().await;
    let one = server
        .mock("DELETE", "/v1/gate/sessions/sess_1")
        .with_status(204)
        .create_async()
        .await;
    let all = server
        .mock(
            "DELETE",
            format!("/v1/gate/identities/{USER}/sessions").as_str(),
        )
        .with_status(204)
        .create_async()
        .await;

    gate.sessions().revoke("sess_1").await.unwrap();
    gate.sessions().revoke_all(USER).await.unwrap();

    one.assert_async().await;
    all.assert_async().await;
}

/// An id that is not one of your users is `403 forbidden` in the envelope.
#[tokio::test]
async fn revoke_all_of_an_identity_not_yours_is_a_403() {
    let (mut server, gate) = gate().await;
    server
        .mock("DELETE", format!("/v1/gate/identities/{USER}/sessions").as_str())
        .with_status(403)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"error":{"code":"forbidden","message":"Identity does not belong to this tenant","request_id":"req_1"}}"#)
        .create_async()
        .await;

    let err = gate.sessions().revoke_all(USER).await.unwrap_err();
    assert!(
        matches!(err, Error::Api(e) if e.status == 403 && e.code == "forbidden" && e.request_id == "req_1")
    );
}

#[tokio::test]
async fn session_check_sends_x_session_token_and_parses_orgs() {
    let (mut server, gate) = gate().await;
    let mock = server
        .mock("GET", "/v1/gate/auth/session")
        .match_header("x-session-token", SESSION_TOKEN)
        .match_header("authorization", format!("Bearer {KEY}").as_str())
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(format!(
            r#"{{"active":true,"aal":"aal1","mfa_required":false,"mfa_satisfied":true,"identity_id":"{USER}","orgs":[{{"id":"{ORG}","role":"admin","permissions":["invoices:write"]}}]}}"#
        ))
        .create_async()
        .await;

    let check = gate.sessions().check(SESSION_TOKEN).await.unwrap();

    assert!(check.active && check.mfa_satisfied && !check.mfa_required);
    assert_eq!(check.identity_id.as_deref(), Some(USER));
    assert_eq!(check.orgs.unwrap()[0].role, "admin");
    assert_eq!(check.orgs_truncated, None);
    mock.assert_async().await;
}

#[tokio::test]
async fn session_check_mfa_required_exposes_the_levels_in_details() {
    let (mut server, gate) = gate().await;
    server
        .mock("GET", "/v1/gate/auth/session")
        .with_status(403)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"error":{"code":"mfa_required","message":"A second factor is required.","request_id":"req_9","required_aal":"aal2","current_aal":"aal1","sign_in":"/v1/gate/auth/login?aal=aal2"}}"#)
        .create_async()
        .await;

    let err = gate.sessions().check(SESSION_TOKEN).await.unwrap_err();
    match err {
        Error::Api(e) => {
            assert_eq!((e.status, e.code.as_str()), (403, "mfa_required"));
            assert_eq!(e.request_id, "req_9");
            assert_eq!(e.details["required_aal"], "aal2");
            assert_eq!(e.details["current_aal"], "aal1");
            // code, message and request_id are not repeated in details
            assert!(!e.details.contains_key("code"));
            assert!(!e.details.contains_key("request_id"));
        }
        other => panic!("expected Error::Api, got {other:?}"),
    }
}

// --- audit log ---------------------------------------------------------------------

#[tokio::test]
async fn audit_log_sends_the_filters_that_are_set() {
    let (mut server, gate) = gate().await;
    let mock = server
        .mock(
            "GET",
            format!(
                "/v1/gate/audit-log?limit=1&action=organization.member_added&actor_type=api_key&target_id={USER}&from=2026-10-14T00%3A00%3A00Z"
            )
            .as_str(),
        )
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(format!(
            r#"{{"data":[{{"id":"a1","occurred_at":"2026-10-14T10:00:00Z","action":"organization.member_added","actor":{{"type":"api_key","id":"key_1"}},"target":{{"type":"identity","id":"{USER}"}},"project_id":null,"details":{{"role":"admin"}},"ip":"203.0.113.9","user_agent":"sdk"}}],"has_more":true,"next_cursor":"nc"}}"#
        ))
        .create_async()
        .await;

    let page = gate
        .audit_log()
        .list(ListAuditLogParams {
            limit: Some(1),
            action: Some("organization.member_added".into()),
            actor_type: Some("api_key".into()),
            target_id: Some(USER.into()),
            from: Some("2026-10-14T00:00:00Z".into()),
            ..Default::default()
        })
        .await
        .unwrap();

    let entry = &page.data[0];
    assert_eq!(entry.action, "organization.member_added");
    assert_eq!(entry.actor.kind, "api_key");
    assert_eq!(entry.actor.id.as_deref(), Some("key_1"));
    let target = entry.target.as_ref().unwrap();
    assert_eq!(
        (target.kind.as_str(), target.id.as_str()),
        ("identity", USER)
    );
    assert_eq!(entry.project_id, None);
    assert_eq!(entry.details["role"], "admin");
    assert_eq!(entry.ip.as_deref(), Some("203.0.113.9"));
    assert!(page.has_more);
    assert_eq!(page.next_cursor.as_deref(), Some("nc"));
    mock.assert_async().await;
}

#[tokio::test]
async fn audit_log_with_no_filters_has_no_query_and_tolerates_null_target_and_actor_id() {
    let (mut server, gate) = gate().await;
    let mock = server
        .mock("GET", "/v1/gate/audit-log")
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"data":[{"id":"a2","occurred_at":"2026-10-14T10:00:00Z","action":"a.new.action","actor":{"type":"unknown","id":null},"target":null,"project_id":null,"details":{},"ip":null,"user_agent":null}],"has_more":false,"next_cursor":null}"#)
        .create_async()
        .await;

    let page = gate
        .audit_log()
        .list(ListAuditLogParams::default())
        .await
        .unwrap();

    assert_eq!(page.data[0].actor.kind, "unknown");
    assert_eq!(page.data[0].actor.id, None);
    assert!(page.data[0].target.is_none());
    mock.assert_async().await;
}

// --- lifecycle ---------------------------------------------------------------------

fn rule_json(kind: &str, enabled: bool) -> String {
    format!(
        r#"{{"kind":"{kind}","configured":true,"enabled":{enabled},"params":{{"delete_after_days":14,"warn_before_days":3}},"available":true,"disabled_reason":null,"enabled_at":null,"updated_at":"2026-10-14T10:00:00Z","pending":0}}"#
    )
}

#[tokio::test]
async fn lifecycle_list_rules() {
    let (mut server, gate) = gate().await;
    let mock = server
        .mock("GET", "/v1/gate/lifecycle/rules")
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(format!(
            r#"{{"tier":"free","rules":[{},{{"kind":"expire_roles","configured":false,"enabled":false,"params":null,"available":false,"disabled_reason":null,"enabled_at":null,"updated_at":null,"pending":0}}]}}"#,
            rule_json("delete_unverified", true)
        ))
        .create_async()
        .await;

    let rules = gate.lifecycle().list_rules().await.unwrap();

    assert_eq!(rules.tier, "free");
    assert_eq!(rules.rules[0].kind, LifecycleRuleKind::DeleteUnverified);
    assert!(rules.rules[0].enabled);
    assert_eq!(
        rules.rules[0].params.as_ref().unwrap()["delete_after_days"],
        14
    );
    assert_eq!(rules.rules[1].kind, LifecycleRuleKind::ExpireRoles);
    assert!(!rules.rules[1].available);
    assert_eq!(rules.rules[1].params, None);
    mock.assert_async().await;
}

#[tokio::test]
async fn lifecycle_set_rule_sends_exactly_enabled_and_params() {
    let (mut server, gate) = gate().await;
    let with_params = server
        .mock("PUT", "/v1/gate/lifecycle/rules/delete_unverified")
        .match_body(Matcher::Json(json!({
            "enabled": false,
            "params": {"delete_after_days": 14, "warn_before_days": 3},
        })))
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(rule_json("delete_unverified", false))
        .create_async()
        .await;
    // `params` omitted (not null) keeps the saved settings; the server refuses unknown fields.
    let without = server
        .mock("PUT", "/v1/gate/lifecycle/rules/delete_unverified")
        .match_body(Matcher::Json(json!({"enabled": true})))
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(rule_json("delete_unverified", true))
        .create_async()
        .await;

    let lifecycle = gate.lifecycle();
    let saved = lifecycle
        .set_rule(
            LifecycleRuleKind::DeleteUnverified,
            SetRuleParams {
                enabled: false,
                params: Some(json!({"delete_after_days": 14, "warn_before_days": 3})),
            },
        )
        .await
        .unwrap();
    assert!(!saved.enabled);

    let enabled = lifecycle
        .set_rule(
            LifecycleRuleKind::DeleteUnverified,
            SetRuleParams {
                enabled: true,
                params: None,
            },
        )
        .await
        .unwrap();
    assert!(enabled.enabled);

    with_params.assert_async().await;
    without.assert_async().await;
}

#[tokio::test]
async fn lifecycle_plan_required_is_a_403_error() {
    let (mut server, gate) = gate().await;
    server
        .mock("PUT", "/v1/gate/lifecycle/rules/remind_unverified")
        .with_status(403)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"error":{"code":"plan_required","message":"This rule is not in your plan.","request_id":"req_4"}}"#)
        .create_async()
        .await;

    let err = gate
        .lifecycle()
        .set_rule(
            LifecycleRuleKind::RemindUnverified,
            SetRuleParams {
                enabled: true,
                params: Some(json!({"remind_after_hours": 24})),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Api(e) if e.status == 403 && e.code == "plan_required"));
}

#[tokio::test]
async fn lifecycle_preview_wraps_params_and_expire_roles_sends_an_empty_object() {
    let (mut server, gate) = gate().await;
    let preview = server
        .mock("POST", "/v1/gate/lifecycle/rules/delete_inactive/preview")
        .match_body(Matcher::Json(json!({
            "params": {"inactive_after_months": 12, "warn_before_days": 14},
        })))
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(
            r#"{"kind":"delete_inactive","available":false,"matching_now":42,"capped":false}"#,
        )
        .create_async()
        .await;
    let expire = server
        .mock("POST", "/v1/gate/lifecycle/rules/expire_roles/preview")
        .match_body(Matcher::Json(json!({"params": {}})))
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"kind":"expire_roles","available":true,"matching_now":0,"capped":false}"#)
        .create_async()
        .await;

    let lifecycle = gate.lifecycle();
    let p = lifecycle
        .preview_rule(
            LifecycleRuleKind::DeleteInactive,
            json!({"inactive_after_months": 12, "warn_before_days": 14}),
        )
        .await
        .unwrap();
    assert_eq!((p.matching_now, p.available, p.capped), (42, false, false));

    let e = lifecycle
        .preview_rule(LifecycleRuleKind::ExpireRoles, json!({}))
        .await
        .unwrap();
    assert_eq!(e.kind, LifecycleRuleKind::ExpireRoles);

    preview.assert_async().await;
    expire.assert_async().await;
}

#[tokio::test]
async fn lifecycle_delete_rule_and_list_actions() {
    let (mut server, gate) = gate().await;
    let delete = server
        .mock("DELETE", "/v1/gate/lifecycle/rules/delete_unverified")
        .with_status(204)
        .create_async()
        .await;
    let actions = server
        .mock(
            "GET",
            format!(
                "/v1/gate/lifecycle/actions?limit=5&status=pending&kind=delete_unverified&identity_id={USER}"
            )
            .as_str(),
        )
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(format!(
            r#"{{"data":[{{"id":"act_1","kind":"delete_unverified","step":"warn","identity_id":"{USER}","status":"pending","basis_at":"2026-10-01T00:00:00Z","due_at":"2026-10-20T00:00:00Z","reason":null,"created_at":"2026-10-14T10:00:00Z","finished_at":null}}],"has_more":false,"next_cursor":null}}"#
        ))
        .create_async()
        .await;

    gate.lifecycle()
        .delete_rule(LifecycleRuleKind::DeleteUnverified)
        .await
        .unwrap();

    let page = gate
        .lifecycle()
        .list_actions(ListLifecycleActionsParams {
            limit: Some(5),
            status: Some("pending".into()),
            kind: Some(LifecycleRuleKind::DeleteUnverified),
            identity_id: Some(USER.into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(page.data[0].step, "warn");
    assert_eq!(page.data[0].kind, LifecycleRuleKind::DeleteUnverified);
    assert_eq!(page.data[0].reason, None);
    delete.assert_async().await;
    actions.assert_async().await;
}

// --- flows, return URLs ------------------------------------------------------------

#[tokio::test]
async fn login_flow_keeps_the_session_token_out_of_the_query() {
    let (mut server, gate) = gate().await;
    let mock = server
        .mock(
            "GET",
            "/v1/gate/auth/login?return_to=https%3A%2F%2Fapp.example.com%2Fcb&state=s-1&aal=aal2",
        )
        .match_header("x-session-token", SESSION_TOKEN)
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"id":"flow_1","ui":{"action":"x","nodes":[]}}"#)
        .create_async()
        .await;

    let flow = gate
        .create_login_flow_with(LoginFlowParams {
            return_to: Some("https://app.example.com/cb".into()),
            state: Some("s-1".into()),
            aal: Some("aal2".into()),
            session_token: Some(SESSION_TOKEN.into()),
        })
        .await
        .unwrap();

    assert_eq!(flow["id"], "flow_1");
    mock.assert_async().await;
}

#[tokio::test]
async fn registration_flow_and_oidc_exchange() {
    let (mut server, gate) = gate().await;
    let reg = server
        .mock("GET", "/v1/gate/auth/registration?state=s-2")
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"id":"flow_2"}"#)
        .create_async()
        .await;
    let exchange = server
        .mock("POST", "/v1/gate/auth/oidc/exchange")
        .match_body(Matcher::Json(json!({"code": "gxc_abc"})))
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(format!(
            r#"{{"session_token":"{SESSION_TOKEN}","session":{}}}"#,
            session_json("sess_2")
        ))
        .create_async()
        .await;

    let flow = gate
        .create_registration_flow(RegistrationFlowParams {
            state: Some("s-2".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(flow["id"], "flow_2");

    let result = gate.exchange_oidc_code("gxc_abc").await.unwrap();
    assert_eq!(result.session_token, SESSION_TOKEN);
    assert_eq!(result.session.id, "sess_2");

    reg.assert_async().await;
    exchange.assert_async().await;
}

#[tokio::test]
async fn return_urls_get_and_put() {
    let (mut server, gate) = gate().await;
    let get = server
        .mock("GET", "/v1/gate/settings/return-urls")
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"return_urls":["https://app.example.com/cb"]}"#)
        .create_async()
        .await;
    let put = server
        .mock("PUT", "/v1/gate/settings/return-urls")
        .match_body(Matcher::Json(
            json!({"return_urls": ["http://localhost:3000/cb"]}),
        ))
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"return_urls":["http://localhost:3000/cb"]}"#)
        .create_async()
        .await;

    assert_eq!(
        gate.settings().get_return_urls().await.unwrap(),
        ["https://app.example.com/cb"]
    );
    assert_eq!(
        gate.settings()
            .update_return_urls(vec!["http://localhost:3000/cb".into()])
            .await
            .unwrap(),
        ["http://localhost:3000/cb"]
    );
    get.assert_async().await;
    put.assert_async().await;
}

#[tokio::test]
async fn update_security_still_sends_both_fields() {
    let (mut server, gate) = gate().await;
    let mock = server
        .mock("PUT", "/v1/gate/settings/security")
        .match_body(Matcher::Json(
            json!({"passwordless_enabled": false, "mfa_enabled": true}),
        ))
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"status":"ok"}"#)
        .create_async()
        .await;

    gate.settings()
        .update_security(SecuritySettings {
            passwordless_enabled: false,
            mfa_enabled: true,
        })
        .await
        .unwrap();
    mock.assert_async().await;
}
