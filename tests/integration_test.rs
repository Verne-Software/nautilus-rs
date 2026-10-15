//! Integration suite: every 1.4.0 resource against a real Nautilus stack.
//!
//! **Opt-in.** Every test here is `#[ignore]`d, so `cargo test` never runs it, and it needs
//! the `jwt` feature (it verifies a signed token). Run it against a LOCAL stack:
//!
//! ```text
//! eval "$(node …/vrn-sdk-node/scripts/local-stack-key.mjs env /tmp/sdk-it.json)"
//! cargo test --features jwt --test integration_test -- --ignored --test-threads=1
//! ```
//!
//! Environment:
//!
//! - `VERNE_API_KEY=vrn_…`: a key with the scopes `gate`, `clockwork`, `passepartout`
//!   (required; the tests fail without it rather than pass by doing nothing).
//! - `VERNE_BASE_URL`: default `http://api.soft.com`. Any host but `api.soft.com`,
//!   `localhost` or `127.0.0.1` is refused unless `VERNE_ALLOW_REMOTE=1`.
//! - `VERNE_EMAIL_PREFIX`: every end user created is `<prefix>-<n>@e2e.soft.com`, so
//!   `make purge-e2e E2E_PREFIXES=<prefix>` sweeps them.
//! - `VERNE_NARROW_KEY`: optional, a key WITHOUT the `gate` scope (checks the `403` envelope).
//!
//! It creates end users, an organization with roles and members, changes the tenant's
//! security settings and return URLs for a moment, and removes all of it again — also when
//! an assertion fails.
#![cfg(feature = "jwt")]

use std::{
    future::Future,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use nautilus_rs::{
    AddMemberParams, ApiError, AuthorizeParams, AuthorizeReason, CreateCronJobParams,
    CreateIdentityParams, CreateOrganizationParams, CreateRoleParams, CreateTokenParams, Error,
    Gate, GateTokenVerifier, IdentityCredentials, IdentityTraitsInput, LifecycleRuleKind,
    ListAuditLogParams, ListLifecycleActionsParams, ListParams, LoginFlowParams, Passepartout,
    SecuritySettings, SetRuleParams, TokenErrorReason, UpdateCronJobParams, UpdateMemberParams,
    UpdateRoleParams, Verne,
};
use serde_json::json;

const NIL_UUID: &str = "00000000-0000-4000-8000-000000000000";

struct Env {
    key: String,
    base_url: String,
    prefix: String,
    narrow_key: Option<String>,
}

/// The environment, or a panic: these tests only run when asked for (`--ignored`), and
/// asked for without a key they must not pass by doing nothing. Also panics on a
/// non-local host.
fn env() -> Env {
    let key = std::env::var("VERNE_API_KEY")
        .expect("VERNE_API_KEY is not set: the integration suite has nothing to run against");
    let base_url = std::env::var("VERNE_BASE_URL")
        .unwrap_or_else(|_| "http://api.soft.com".into())
        .trim_end_matches('/')
        .to_owned();
    let host = base_url
        .split("://")
        .nth(1)
        .unwrap_or(&base_url)
        .split(['/', ':'])
        .next()
        .unwrap_or_default();
    if !["api.soft.com", "localhost", "127.0.0.1"].contains(&host)
        && std::env::var("VERNE_ALLOW_REMOTE").as_deref() != Ok("1")
    {
        panic!("refusing to run the integration suite against {host}; set VERNE_ALLOW_REMOTE=1 to insist");
    }
    Env {
        key,
        base_url,
        prefix: std::env::var("VERNE_EMAIL_PREFIX")
            .unwrap_or_else(|_| format!("e2e-sdkrs-{:x}", unix_now())),
        narrow_key: std::env::var("VERNE_NARROW_KEY").ok(),
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// RFC 3339 UTC for a Unix time (Howard Hinnant's civil-from-days).
fn rfc3339(secs: u64) -> String {
    let (days, rem) = ((secs / 86_400) as i64, secs % 86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn password() -> String {
    format!("Pw-{:x}-{:x}-Zq9!x", unix_now(), std::process::id())
}

fn api(result: Result<impl std::fmt::Debug, Error>, status: u16, code: Option<&str>) -> ApiError {
    match result {
        Err(Error::Api(e)) => {
            assert_eq!(e.status, status, "status of {e}");
            if let Some(code) = code {
                assert_eq!(e.code, code, "code of {e}");
            }
            *e
        }
        other => panic!("expected a {status}, got {other:?}"),
    }
}

/// A password login through the flow's `ui.action` — the part an SDK leaves to the browser.
async fn password_login(gate: &Gate, identifier: &str, secret: &str) -> String {
    let flow = gate.create_login_flow().await.unwrap();
    let action = flow["ui"]["action"].as_str().expect("flow ui.action");
    let res = reqwest::Client::new()
        .post(action)
        .header("Accept", "application/json")
        .json(&json!({"method": "password", "identifier": identifier, "password": secret}))
        .send()
        .await
        .unwrap();
    let status = res.status();
    let body: serde_json::Value = res.json().await.unwrap();
    body["session_token"]
        .as_str()
        .unwrap_or_else(|| panic!("login failed: HTTP {status}"))
        .to_owned()
}

/// What the suite created, so it can be removed even when an assertion fails.
#[derive(Default)]
struct Created {
    identities: Vec<String>,
    organization: Option<String>,
    lifecycle_rule: bool,
    security: Option<SecuritySettings>,
    return_urls: Option<Vec<String>>,
}

/// Run `body` on its own task; whatever it panics with, clean up, then rethrow.
async fn with_cleanup<F, Fut>(gate: &Gate, body: F)
where
    F: FnOnce(Arc<Mutex<Created>>) -> Fut,
    Fut: Future<Output = ()> + Send + 'static,
{
    let created = Arc::new(Mutex::new(Created::default()));
    let outcome = tokio::spawn(body(Arc::clone(&created))).await;

    let c = std::mem::take(&mut *created.lock().unwrap());
    if let Some(org) = c.organization {
        let _ = gate.organizations().delete(&org).await;
    }
    for id in c.identities {
        let _ = gate.identities().delete(&id).await;
    }
    if c.lifecycle_rule {
        let _ = gate
            .lifecycle()
            .delete_rule(LifecycleRuleKind::DeleteUnverified)
            .await;
    }
    if let Some(security) = c.security {
        let _ = gate.settings().update_security(security).await;
    }
    if let Some(urls) = c.return_urls {
        let _ = gate.settings().update_return_urls(urls).await;
    }

    if let Err(e) = outcome {
        std::panic::resume_unwind(e.into_panic());
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a running local stack and VERNE_API_KEY; see the module docs"]
async fn gate_against_a_live_stack() {
    let env = env();
    let gate = |env: &Env| {
        Gate::builder()
            .api_key(&env.key)
            .base_url(&env.base_url)
            .build()
            .unwrap()
    };
    let cleaner = gate(&env);

    with_cleanup(&cleaner, move |created| {
        let gate = gate(&env);
        async move { gate_suite(gate, env, created).await }
    })
    .await;
}

async fn gate_suite(gate: Gate, env: Env, created: Arc<Mutex<Created>>) {
    let email = |n: &str| format!("{}-{n}@e2e.soft.com", env.prefix);
    let created_identity = |id: &str| created.lock().unwrap().identities.push(id.to_owned());

    // --- tokens.create: the key in the header, the tenant in the answer -------------------
    let token = gate
        .tokens()
        .create(CreateTokenParams {
            subject: "sdk-integration".into(),
            scopes: None,
            ttl_seconds: None,
        })
        .await
        .unwrap();
    assert!(
        token.access_token.starts_with("gat_"),
        "{}",
        token.access_token
    );
    let tenant_id = token.tenant_id;
    assert_eq!(tenant_id.len(), 36);

    // --- a key without the gate scope gets the standard 403 envelope ----------------------
    if let Some(narrow) = &env.narrow_key {
        let narrow = Gate::builder()
            .api_key(narrow)
            .base_url(&env.base_url)
            .build()
            .unwrap();
        let e = api(
            narrow.organizations().list(ListParams::default()).await,
            403,
            Some("forbidden"),
        );
        assert_eq!(e.message, "This API key is not authorized for Gate.");
        assert!(e.request_id.starts_with("req_"), "{}", e.request_id);
    }

    // --- end users ------------------------------------------------------------------------
    let member_password = password();
    let member = gate
        .identities()
        .create(CreateIdentityParams {
            schema_id: "user".into(),
            traits: IdentityTraitsInput {
                email: email("member"),
                locale: None,
            },
            credentials: Some(IdentityCredentials::password(member_password.clone())),
            state: None,
            ..Default::default()
        })
        .await
        .unwrap();
    created_identity(&member.id);
    let outsider = gate
        .identities()
        .create(CreateIdentityParams {
            schema_id: "user".into(),
            traits: IdentityTraitsInput {
                email: email("outsider"),
                locale: None,
            },
            credentials: None,
            state: None,
            ..Default::default()
        })
        .await
        .unwrap();
    created_identity(&outsider.id);
    assert_eq!(member.traits.email, email("member"));

    // 409 is "you already have this end user".
    api(
        gate.identities()
            .create(CreateIdentityParams {
                schema_id: "user".into(),
                traits: IdentityTraitsInput {
                    email: email("member"),
                    locale: None,
                },
                credentials: None,
                state: None,
                ..Default::default()
            })
            .await,
        409,
        Some("conflict"),
    );

    // --- custom metadata: read, merge, delete a key --------------------------------------
    assert!(gate
        .identities()
        .get_metadata(&member.id)
        .await
        .unwrap()
        .is_empty());
    let merged = gate
        .identities()
        .update_metadata(&member.id, json!({"plan": "pro", "seats": 5}))
        .await
        .unwrap();
    assert_eq!(
        serde_json::Value::Object(merged),
        json!({"plan": "pro", "seats": 5})
    );
    let merged = gate
        .identities()
        .update_metadata(&member.id, json!({"seats": null}))
        .await
        .unwrap();
    assert_eq!(serde_json::Value::Object(merged), json!({"plan": "pro"}));

    // --- organizations --------------------------------------------------------------------
    let mut org = gate
        .organizations()
        .create(CreateOrganizationParams {
            name: "Acme".into(),
        })
        .await
        .unwrap();
    created.lock().unwrap().organization = Some(org.id.clone());
    assert_eq!(org.name, "Acme");
    let listed = gate
        .organizations()
        .list(ListParams {
            limit: Some(100),
            cursor: None,
        })
        .await
        .unwrap();
    assert!(listed.data.iter().any(|o| o.id == org.id));
    assert_eq!(gate.organizations().get(&org.id).await.unwrap().id, org.id);
    org = gate
        .organizations()
        .update(
            &org.id,
            nautilus_rs::UpdateOrganizationParams {
                name: "Acme Corp".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(org.name, "Acme Corp");
    api(
        gate.organizations()
            .create(CreateOrganizationParams {
                name: String::new(),
            })
            .await,
        400,
        Some("invalid_payload"),
    );

    // --- roles ----------------------------------------------------------------------------
    let roles = gate.organizations().roles();
    let admin = roles
        .create(
            &org.id,
            CreateRoleParams {
                name: "admin".into(),
                permissions: Some(vec![
                    "invoices:write".into(),
                    "invoices:read".into(),
                    "invoices:read".into(),
                ]),
            },
        )
        .await
        .unwrap();
    assert_eq!(admin.permissions, ["invoices:read", "invoices:write"]);
    let viewer = roles
        .create(
            &org.id,
            CreateRoleParams {
                name: "viewer".into(),
                permissions: None,
            },
        )
        .await
        .unwrap();
    let viewer = roles
        .update(
            &org.id,
            &viewer.id,
            UpdateRoleParams {
                permissions: Some(vec!["invoices:read".into()]),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(viewer.permissions, ["invoices:read"]);

    let first = roles
        .list(
            &org.id,
            ListParams {
                limit: Some(1),
                cursor: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(first.data.len(), 1);
    assert!(first.has_more);
    let second = roles
        .list(
            &org.id,
            ListParams {
                limit: Some(1),
                cursor: first.next_cursor.clone(),
            },
        )
        .await
        .unwrap();
    assert_ne!(second.data[0].id, first.data[0].id);
    assert_eq!(roles.get(&org.id, &admin.id).await.unwrap().name, "admin");
    api(
        roles
            .create(
                &org.id,
                CreateRoleParams {
                    name: "admin".into(),
                    permissions: None,
                },
            )
            .await,
        409,
        Some("conflict"),
    );
    api(
        roles
            .create(
                &org.id,
                CreateRoleParams {
                    name: "Admin".into(),
                    permissions: None,
                },
            )
            .await,
        400,
        Some("invalid_payload"),
    );

    // --- members --------------------------------------------------------------------------
    let members = gate.organizations().members();
    let added = members
        .add(
            &org.id,
            AddMemberParams {
                identity_id: member.id.clone(),
                role_id: admin.id.clone(),
                expires_at: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(added.expires_at, None);
    api(
        members
            .add(
                &org.id,
                AddMemberParams {
                    identity_id: member.id.clone(),
                    role_id: viewer.id.clone(),
                    expires_at: None,
                },
            )
            .await,
        409,
        Some("conflict"),
    );

    // Absent keeps the deadline, a time sets it, null removes it.
    let deadline = rfc3339(unix_now() + 30 * 24 * 3600);
    let dated = members
        .update(
            &org.id,
            &member.id,
            UpdateMemberParams {
                role_id: None,
                expires_at: Some(Some(deadline.clone())),
            },
        )
        .await
        .unwrap();
    assert_eq!(dated.expires_at.as_deref(), Some(deadline.as_str()));
    let kept = members
        .update(
            &org.id,
            &member.id,
            UpdateMemberParams {
                role_id: Some(admin.id.clone()),
                expires_at: None,
            },
        )
        .await
        .unwrap();
    assert!(kept.expires_at.is_some());
    let cleared = members
        .update(
            &org.id,
            &member.id,
            UpdateMemberParams {
                role_id: None,
                expires_at: Some(None),
            },
        )
        .await
        .unwrap();
    assert_eq!(cleared.expires_at, None);

    let listed = members.list(&org.id, ListParams::default()).await.unwrap();
    assert_eq!(
        listed
            .data
            .iter()
            .map(|m| m.identity_id.as_str())
            .collect::<Vec<_>>(),
        [member.id.as_str()]
    );
    let held = gate
        .identities()
        .list_organizations(&member.id, ListParams::default())
        .await
        .unwrap();
    assert_eq!(held.data.len(), 1);
    assert_eq!(
        (held.data[0].id.as_str(), held.data[0].role.as_str()),
        (org.id.as_str(), "admin")
    );
    assert_eq!(
        held.data[0].permissions,
        ["invoices:read", "invoices:write"]
    );
    assert!(gate
        .identities()
        .list_organizations(&outsider.id, ListParams::default())
        .await
        .unwrap()
        .data
        .is_empty());

    // A role somebody holds cannot be deleted.
    let e = api(
        roles.delete(&org.id, &admin.id).await,
        409,
        Some("conflict"),
    );
    assert!(e.message.contains("held by 1 member"), "{}", e.message);

    // --- authorize / can: every reason the organization check gives ----------------------
    let orgs = gate.organizations();
    let yes = orgs
        .authorize(&org.id, &member.id, "invoices:write")
        .await
        .unwrap();
    assert!(yes.allowed);
    assert_eq!(yes.reason, AuthorizeReason::MemberWithPermission);
    assert_eq!(yes.role.as_deref(), Some("admin"));
    assert!(yes.decision_id.starts_with("dec_"), "{}", yes.decision_id);

    let no = orgs
        .authorize(&org.id, &member.id, "members:invite")
        .await
        .unwrap();
    assert!(!no.allowed);
    assert_eq!(no.reason, AuthorizeReason::MemberWithoutPermission);
    assert_eq!(no.role.as_deref(), Some("admin"));

    let stranger = orgs
        .authorize(&org.id, &outsider.id, "invoices:read")
        .await
        .unwrap();
    assert!(!stranger.allowed);
    assert_eq!(stranger.reason, AuthorizeReason::NotAMember);
    assert_eq!(stranger.role, None);

    let missing = orgs
        .authorize(NIL_UUID, &member.id, "invoices:read")
        .await
        .unwrap();
    assert!(!missing.allowed);
    assert_eq!(missing.reason, AuthorizeReason::NoSuchOrganization);

    gate.identities().deactivate(&member.id).await.unwrap();
    let inactive = orgs.authorize(&org.id, &member.id, "invoices:write").await;
    gate.identities().activate(&member.id).await.unwrap();
    let inactive = inactive.unwrap();
    assert!(!inactive.allowed);
    assert_eq!(inactive.reason, AuthorizeReason::SubjectInactive);
    assert_eq!(inactive.role.as_deref(), Some("admin"));

    assert!(orgs
        .can(&org.id, &member.id, "invoices:write")
        .await
        .unwrap());
    assert!(!orgs
        .can(&org.id, &member.id, "members:invite")
        .await
        .unwrap());
    api(
        orgs.can(&org.id, &member.id, "Invoices:*").await,
        400,
        Some("invalid_payload"),
    );

    // The plain ownership check: a code in `reason`, the sentence in `message`.
    let owns = |subject: &str| {
        gate.authorize(AuthorizeParams {
            subject: subject.to_owned(),
            action: "read".into(),
            resource: "doc:1".into(),
            context: None,
        })
    };
    let owned = owns(&member.id).await.unwrap();
    assert!(owned.allowed);
    assert_eq!(owned.reason, AuthorizeReason::OwnedByTenant);
    assert!(
        owned
            .message
            .as_deref()
            .is_some_and(|m| m.contains("is an identity owned by tenant")),
        "{:?}",
        owned.message
    );
    let nobody = owns(NIL_UUID).await.unwrap();
    assert!(!nobody.allowed);
    assert_eq!(nobody.reason, AuthorizeReason::NotAKnownIdentity);
    let not_an_id = owns("usr_123").await.unwrap();
    assert_eq!(not_an_id.reason, AuthorizeReason::NotAKnownIdentity);
    gate.identities().deactivate(&outsider.id).await.unwrap();
    let off = owns(&outsider.id).await;
    gate.identities().activate(&outsider.id).await.unwrap();
    let off = off.unwrap();
    assert!(!off.allowed);
    assert_eq!(off.reason, AuthorizeReason::SubjectInactive);

    // --- a malformed token request is 400 invalid_payload, with a request id -------------
    let e = api(
        gate.tokens()
            .create(CreateTokenParams {
                subject: String::new(),
                scopes: None,
                ttl_seconds: None,
            })
            .await,
        400,
        Some("invalid_payload"),
    );
    assert!(e.request_id.starts_with("req_"), "{}", e.request_id);

    // --- the identity object no longer carries the owner stamp ---------------------------
    let read = gate.identities().get(&member.id).await.unwrap();
    assert!(
        read.metadata_public
            .as_ref()
            .and_then(|m| m.get("verne_tenant_id"))
            .is_none(),
        "{:?}",
        read.metadata_public
    );
    assert!(!read.extra.contains_key("verne_tenant_id"));

    // --- introspection lists the end user's organizations --------------------------------
    let token = gate
        .tokens()
        .create(CreateTokenParams {
            subject: member.id.clone(),
            scopes: None,
            ttl_seconds: None,
        })
        .await
        .unwrap();
    let info = gate.tokens().introspect(&token.access_token).await.unwrap();
    assert!(info.active);
    let org_claims = info.orgs.expect("orgs for an end user");
    assert_eq!(org_claims.len(), 1);
    assert_eq!(org_claims[0].id, org.id);
    let dead = gate
        .tokens()
        .introspect("gat_live_not_a_token")
        .await
        .unwrap();
    assert!(!dead.active);
    assert_eq!(dead.subject, None);
    assert_eq!(dead.orgs, None);

    // --- sessions: check, mfa_required, signed token, verify, revoke ---------------------
    let session_token = password_login(&gate, &email("member"), &member_password).await;

    let check = gate.sessions().check(&session_token).await.unwrap();
    assert!(check.active && check.mfa_satisfied);
    assert_eq!(check.identity_id.as_deref(), Some(member.id.as_str()));
    assert_eq!(check.orgs.expect("orgs in the session check")[0].id, org.id);

    let security = gate.settings().get_security().await.unwrap();
    created.lock().unwrap().security = Some(security.clone());
    gate.settings()
        .update_security(SecuritySettings {
            mfa_enabled: true,
            ..security.clone()
        })
        .await
        .unwrap();
    let mfa = api(
        gate.sessions().check(&session_token).await,
        403,
        Some("mfa_required"),
    );
    assert_eq!(mfa.details["required_aal"], "aal2");
    assert_eq!(mfa.details["current_aal"], "aal1");
    gate.settings().update_security(security).await.unwrap();
    created.lock().unwrap().security = None;

    let signed = gate.tokens().create_signed(&session_token).await.unwrap();
    assert!(!signed.session.id.is_empty());
    let claims = gate
        .tokens()
        .verify_signed(&signed.token, &tenant_id)
        .await
        .unwrap();
    assert_eq!(claims.sub, member.id);
    assert_eq!(claims.tenant_id, tenant_id);
    assert_eq!(claims.role, "authenticated");
    assert_eq!(claims.sid, signed.session.id);
    assert_eq!(claims.orgs.len(), 1);
    assert_eq!(claims.orgs[0].id, org.id);
    assert_eq!(claims.iss, format!("{}/auth/{tenant_id}", env.base_url));

    // Another tenant id means another issuer: the same token is refused.
    let foreign = GateTokenVerifier::builder()
        .tenant_id(NIL_UUID)
        .base_url(&env.base_url)
        .build()
        .unwrap();
    match foreign.verify(&signed.token).await {
        // either its key set does not exist, or the issuer does not match
        Err(Error::Token(e)) => assert!(
            matches!(
                e.reason,
                TokenErrorReason::WrongIssuer | TokenErrorReason::JwksUnavailable
            ),
            "{e}"
        ),
        other => panic!("a foreign tenant's token must be refused, got {other:?}"),
    }

    let sessions = gate.sessions().list(&member.id).await.unwrap();
    assert!(sessions.iter().any(|s| s.id == signed.session.id));

    gate.sessions().revoke(&signed.session.id).await.unwrap();
    gate.sessions().revoke(&signed.session.id).await.unwrap(); // idempotent
    api(gate.tokens().create_signed(&session_token).await, 401, None);
    api(
        gate.sessions().check(&session_token).await,
        401,
        Some("unauthorized"),
    );
    api(
        gate.sessions().revoke(NIL_UUID).await,
        404,
        Some("not_found"),
    );

    // "Log out everywhere" is idempotent: nothing left to revoke, a user who never signed
    // in, twice over — all Ok. An id that is not one of your users (unknown, or deleted
    // through the API) is 403.
    gate.sessions().revoke_all(&member.id).await.unwrap();
    gate.sessions().revoke_all(&outsider.id).await.unwrap();
    gate.sessions().revoke_all(&outsider.id).await.unwrap();
    api(
        gate.sessions().revoke_all(NIL_UUID).await,
        403,
        Some("forbidden"),
    );
    let gone = gate
        .identities()
        .create(CreateIdentityParams {
            schema_id: "user".into(),
            traits: IdentityTraitsInput {
                email: email("gone"),
                locale: None,
            },
            credentials: None,
            state: None,
            ..Default::default()
        })
        .await
        .unwrap();
    created_identity(&gone.id);
    gate.identities().delete(&gone.id).await.unwrap();
    api(
        gate.sessions().revoke_all(&gone.id).await,
        403,
        Some("forbidden"),
    );

    // --- audit log records the role and membership changes, filtered and paged -----------
    let added = gate
        .audit_log()
        .list(ListAuditLogParams {
            action: Some("organization.member_added".into()),
            target_id: Some(member.id.clone()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(added.data.len(), 1);
    assert_eq!(added.data[0].action, "organization.member_added");
    assert_eq!(added.data[0].actor.kind, "api_key");
    let target = added.data[0].target.as_ref().expect("target");
    assert_eq!(
        (target.kind.as_str(), target.id.as_str()),
        ("identity", member.id.as_str())
    );

    let since = rfc3339(unix_now() - 3600);
    let page1 = gate
        .audit_log()
        .list(ListAuditLogParams {
            limit: Some(1),
            from: Some(since.clone()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(page1.data.len(), 1);
    assert!(page1.has_more);
    let page2 = gate
        .audit_log()
        .list(ListAuditLogParams {
            limit: Some(1),
            cursor: page1.next_cursor.clone(),
            from: Some(since),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_ne!(page2.data[0].id, page1.data[0].id);
    api(
        gate.audit_log()
            .list(ListAuditLogParams {
                action: Some("nope".into()),
                ..Default::default()
            })
            .await,
        400,
        Some("invalid_payload"),
    );

    // --- lifecycle: list, set (off), preview, delete, actions; plan_required on Free -----
    let rules = gate.lifecycle().list_rules().await.unwrap();
    let mut kinds: Vec<_> = rules.rules.iter().map(|r| r.kind.as_str()).collect();
    kinds.sort_unstable();
    assert_eq!(
        kinds,
        [
            "delete_inactive",
            "delete_unverified",
            "expire_roles",
            "remind_unverified"
        ]
    );

    created.lock().unwrap().lifecycle_rule = true;
    let saved = gate
        .lifecycle()
        .set_rule(
            LifecycleRuleKind::DeleteUnverified,
            SetRuleParams {
                enabled: false,
                params: Some(json!({"delete_after_days": 14, "warn_before_days": 3})),
            },
        )
        .await
        .unwrap();
    assert_eq!(saved.kind, LifecycleRuleKind::DeleteUnverified);
    assert!(saved.configured && !saved.enabled);
    assert_eq!(
        saved.params,
        Some(json!({"delete_after_days": 14, "warn_before_days": 3}))
    );

    let preview = gate
        .lifecycle()
        .preview_rule(
            LifecycleRuleKind::DeleteUnverified,
            json!({"delete_after_days": 14, "warn_before_days": 3}),
        )
        .await
        .unwrap();
    assert_eq!(preview.kind, LifecycleRuleKind::DeleteUnverified);
    assert!(!preview.capped);
    let expire = gate
        .lifecycle()
        .preview_rule(LifecycleRuleKind::ExpireRoles, json!({}))
        .await
        .unwrap();
    assert_eq!(expire.kind, LifecycleRuleKind::ExpireRoles);

    gate.lifecycle()
        .delete_rule(LifecycleRuleKind::DeleteUnverified)
        .await
        .unwrap();
    created.lock().unwrap().lifecycle_rule = false;
    gate.lifecycle()
        .list_actions(ListLifecycleActionsParams {
            limit: Some(5),
            ..Default::default()
        })
        .await
        .unwrap();

    if rules.tier == "free" {
        api(
            gate.lifecycle()
                .set_rule(
                    LifecycleRuleKind::RemindUnverified,
                    SetRuleParams {
                        enabled: true,
                        params: Some(json!({"remind_after_hours": 24})),
                    },
                )
                .await,
            403,
            Some("plan_required"),
        );
    }
    api(
        gate.lifecycle()
            .set_rule(
                LifecycleRuleKind::DeleteUnverified,
                SetRuleParams {
                    enabled: true,
                    params: Some(json!({"delete_after_days": 1, "warn_before_days": 1})),
                },
            )
            .await,
        400,
        Some("invalid_payload"),
    );

    // --- return URLs round-trip and drive the login flow ---------------------------------
    let before = gate.settings().get_return_urls().await.unwrap();
    created.lock().unwrap().return_urls = Some(before.clone());
    let urls = gate
        .settings()
        .update_return_urls(vec!["http://localhost:3000/auth/callback".into()])
        .await
        .unwrap();
    assert_eq!(urls, ["http://localhost:3000/auth/callback"]);
    let flow = gate
        .create_login_flow_with(LoginFlowParams {
            return_to: Some("http://localhost:3000/auth/callback".into()),
            state: Some("sdk-integration-1".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(flow["id"].is_string());
    api(
        gate.create_login_flow_with(LoginFlowParams {
            return_to: Some("https://elsewhere.example/cb".into()),
            ..Default::default()
        })
        .await,
        400,
        None,
    );
    gate.settings().update_return_urls(before).await.unwrap();
    created.lock().unwrap().return_urls = None;
    api(gate.exchange_oidc_code("gxc_not_a_code").await, 404, None);

    // --- removing a member, a role and the organization ----------------------------------
    members.remove(&org.id, &member.id).await.unwrap();
    assert!(!orgs
        .can(&org.id, &member.id, "invoices:read")
        .await
        .unwrap());
    roles.delete(&org.id, &admin.id).await.unwrap();
    roles.delete(&org.id, &viewer.id).await.unwrap();
    orgs.delete(&org.id).await.unwrap();
    created.lock().unwrap().organization = None;
    api(orgs.get(&org.id).await, 404, Some("not_found"));
}

#[tokio::test]
#[ignore = "needs a running local stack and VERNE_API_KEY; see the module docs"]
async fn clockwork_and_passepartout_against_a_live_stack() {
    let env = env();
    let verne = Verne::builder()
        .api_key(&env.key)
        .base_url(&env.base_url)
        .build()
        .unwrap();

    // usage() reports the plan and its limits.
    let usage = verne.clockwork().unwrap().usage().await.unwrap();
    assert!(!usage.plan.is_empty());
    let _ = (usage.cron.active, usage.cron.limit, usage.delayed.pending);

    // An engine error is the standard envelope, with a request id.
    let e = api(
        verne
            .clockwork()
            .unwrap()
            .jobs()
            .create(CreateCronJobParams {
                name: "sdk-bad".into(),
                schedule: "not a schedule".into(),
                url: "https://example.com".into(),
                ..Default::default()
            })
            .await,
        400,
        Some("invalid_payload"),
    );
    assert!(e.message.to_lowercase().contains("cron"), "{}", e.message);
    assert!(e.request_id.starts_with("req_"), "{}", e.request_id);

    // Passepartout introspection of a token that is not one: inactive, nothing else.
    let pp: &Passepartout = verne.passepartout().unwrap();
    let info = pp.introspect("not-a-token").await.unwrap();
    assert!(!info.active);
    assert_eq!(info.orgs, None);

    // A key without the clockwork scope is the standard 403 envelope.
    if let Some(narrow) = &env.narrow_key {
        let narrow = Verne::builder()
            .api_key(narrow)
            .base_url(&env.base_url)
            .build()
            .unwrap();
        // The narrow key has the clockwork scope only, so Gate is refused:
        api(
            narrow
                .gate()
                .unwrap()
                .audit_log()
                .list(ListAuditLogParams::default())
                .await,
            403,
            Some("forbidden"),
        );
    }
}

/// 1.5.0: a cron job keeps its time zone across create, list and update.
#[tokio::test]
#[ignore = "needs a running local stack and VERNE_API_KEY; see the module docs"]
async fn clockwork_timezones_against_a_live_stack() {
    let env = env();
    let verne = Verne::builder()
        .api_key(&env.key)
        .base_url(&env.base_url)
        .build()
        .unwrap();
    let jobs = verne.clockwork().unwrap().jobs();
    let at_utc = |ts: &Option<String>, hour: &str| {
        let ts = ts.as_deref().unwrap_or_default();
        ts.contains(&format!("T{hour}:00:00")) && (ts.ends_with('Z') || ts.ends_with("+00:00"))
    };

    let job = jobs
        .create(CreateCronJobParams {
            name: format!("{}-tz", env.prefix),
            schedule: "0 9 * * *".into(),
            timezone: Some("Europe/Paris".into()),
            url: "https://example.com/hooks/tz".into(),
            method: Some("GET".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    let listed = jobs
        .list()
        .await
        .map(|all| all.into_iter().find(|j| j.id == job.id).map(|j| j.timezone));
    let moved = jobs
        .update(
            &job.id,
            UpdateCronJobParams {
                timezone: Some("America/New_York".into()),
                ..Default::default()
            },
        )
        .await;
    let unknown = jobs
        .update(
            &job.id,
            UpdateCronJobParams {
                timezone: Some("Mars/Olympus".into()),
                ..Default::default()
            },
        )
        .await;
    jobs.delete(&job.id).await.unwrap();

    assert_eq!(job.timezone, "Europe/Paris");
    // 09:00 in Paris in October (CEST, UTC+2) is 07:00 UTC.
    assert!(at_utc(&job.next_run_at, "07"), "{:?}", job.next_run_at);
    assert_eq!(listed.unwrap().as_deref(), Some("Europe/Paris"));
    let moved = moved.unwrap();
    assert_eq!(moved.timezone, "America/New_York");
    // 09:00 in New York in October (EDT, UTC-4) is 13:00 UTC.
    assert!(at_utc(&moved.next_run_at, "13"), "{:?}", moved.next_run_at);
    api(unknown, 400, Some("invalid_payload"));

    // Without a zone a job is UTC.
    let utc = jobs
        .create(CreateCronJobParams {
            name: format!("{}-utc", env.prefix),
            schedule: "0 9 * * *".into(),
            url: "https://example.com/hooks/utc".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    jobs.delete(&utc.id).await.unwrap();
    assert_eq!(utc.timezone, "UTC");
}

/// 1.5.0: an imported user — bcrypt hash, confirmed address, metadata — signs in with the
/// original password.
#[tokio::test]
#[ignore = "needs a running local stack and VERNE_API_KEY; see the module docs"]
async fn identity_import_against_a_live_stack() {
    // bcrypt (cost 10) of IMPORTED_PASSWORD — a test value, nothing else.
    const IMPORTED_PASSWORD: &str = "Imported-Pw-104!";
    const IMPORTED_HASH: &str = "$2b$10$Ky/GcB0gC9sEpv84tO7.Wuxp7RsfSWrDYtz7M5iU4akOb3O5Y1Kq2";

    let env = env();
    let verne = Verne::builder()
        .api_key(&env.key)
        .base_url(&env.base_url)
        .build()
        .unwrap();
    let gate = verne.gate().unwrap();
    let address = format!("{}-import@e2e.soft.com", env.prefix);

    let identity = gate
        .identities()
        .create(CreateIdentityParams {
            schema_id: "user".into(),
            traits: IdentityTraitsInput {
                email: address.clone(),
                locale: None,
            },
            credentials: Some(IdentityCredentials::hashed_password(IMPORTED_HASH)),
            email_verified: Some(true),
            metadata_public: Some(json!({ "previous_user_id": "auth0|sdk-104" })),
            ..Default::default()
        })
        .await
        .unwrap();
    let fetched = gate.identities().get(&identity.id).await;
    let session_token = {
        let flow = gate.create_login_flow().await.unwrap();
        let action = flow["ui"]["action"]
            .as_str()
            .expect("flow ui.action")
            .to_owned();
        let res = reqwest::Client::new()
            .post(action)
            .header("Accept", "application/json")
            .json(&json!({"method": "password", "identifier": address, "password": IMPORTED_PASSWORD}))
            .send()
            .await
            .unwrap();
        let status = res.status();
        let body: serde_json::Value = res.json().await.unwrap();
        (status, body["session_token"].as_str().map(str::to_owned))
    };
    gate.identities().delete(&identity.id).await.unwrap();

    let address = &identity.verifiable_addresses[0];
    assert!(address.verified);
    assert_eq!(address.status.as_deref(), Some("completed"));
    assert_eq!(
        identity.metadata_public,
        Some(json!({ "previous_user_id": "auth0|sdk-104" }))
    );
    assert!(fetched.unwrap().verifiable_addresses[0].verified);
    assert_eq!(
        session_token.0.as_u16(),
        200,
        "login with the original password"
    );
    assert!(session_token.1.is_some());
}
