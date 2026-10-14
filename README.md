# Verne Software Rust SDK

[![Crates.io](https://img.shields.io/crates/v/nautilus-rs.svg)](https://crates.io/crates/nautilus-rs)
[![docs.rs](https://img.shields.io/docsrs/nautilus-rs)](https://docs.rs/nautilus-rs)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](#)

The official Rust SDK for the [Verne Nautilus](https://vernesoft.com) platform: Relay (webhooks), Gate (auth, organizations, sessions, signed tokens), Passepartout (Telegram sign-in) and Clockwork (cron).

> **Server-side only.** API keys carry full access to the services their scopes name and must never be used in WASM or client-side contexts.

The crate is `nautilus-rs`; in code it is `nautilus_rs`.

## Requirements

Rust 1.75 or later, and a Tokio runtime.

## Installation

```toml
[dependencies]
nautilus-rs = "1.4"
tokio = { version = "1", features = ["full"] }

# to verify Gate's signed tokens (JWT) offline:
# nautilus-rs = { version = "1.4", features = ["jwt"] }
```

## API keys

A key is `vrn_…` and carries a set of **scopes** — `relay`, `gate`, `clockwork`, `passepartout` — chosen when you create it on Dashboard → Keys. Pass **one key** and use every service its scopes allow:

```rust
use nautilus_rs::Verne;

#[tokio::main]
async fn main() -> Result<(), nautilus_rs::Error> {
    let verne = Verne::builder()
        .api_key(std::env::var("VERNE_API_KEY").unwrap()) // vrn_…
        .build()?;

    // verne.relay()?, verne.gate()?, verne.passepartout()?, verne.clockwork()?
    Ok(())
}
```

- The per-service setters (`.relay(key)`, `.gate(key)`, `.passepartout(key)`, `.clockwork(key)`) still exist. Use them only to give a service a different key; they **override** `api_key` for that service.
- The SDK does not check scopes and does not parse the key. A call to a service the key has no scope for is `403 forbidden` from the API, with a message naming the service.
- **There is no test/live environment and no sandbox.** Use a separate key per environment (development, staging, production).
- Older keys spelled `vrn_<engine>_…` (`vrn_relay_…`, `vrn_gate_…`) keep working and open that one engine.

You can also use one service on its own:

```rust
use nautilus_rs::{Gate, Relay};

let relay = Relay::new("vrn_9f8a7c1b2d3e4f50");
let gate = Gate::builder().api_key("vrn_9f8a7c1b2d3e4f50").timeout_secs(10).build()?;
```

## Errors

Every call returns `Result<T, nautilus_rs::Error>`. An error answered by the API is `Error::Api(ApiError)`: branch on `status` and `code`. Codes are strings and an open set — keep a catch-all arm.

```rust
use nautilus_rs::Error;

match verne.gate()?.identities().get("idn_…").await {
    Ok(identity) => println!("{}", identity.traits.email),
    Err(Error::Api(e)) if e.status == 404 => println!("no such user"),
    Err(Error::Api(e)) if e.status == 403 && e.code == "mfa_required" => {
        println!("second factor owed: {:?}", e.details.get("required_aal"));
    }
    Err(Error::Api(e)) => {
        eprintln!("{} {}: {} (request {})", e.status, e.code, e.message, e.request_id);
        eprintln!("extra members: {:?}", e.details); // plan, limit, current, kind, …
        eprintln!("raw body: {:?}", e.body);
    }
    Err(Error::Http(e)) => eprintln!("network error: {e}"),
    Err(Error::Token(e)) => eprintln!("untrusted token: {} ({})", e.message, e.reason),
    Err(e) => eprintln!("error: {e}"),
}
```

`ApiError` has `code`, `message`, `status`, `request_id` (empty when the response had none), `details` (every member of `error` besides `code`/`message`/`request_id`), `retry_after` (seconds, from the `Retry-After` header) and `body` (the raw parsed body). Every API route, Clockwork's included, answers in the standard envelope with a string `code` and a `request_id`. Parsing never fails: a body in another shape (one that did not come from the API — a proxy in between, an identity-service answer passed through on `/auth/*`) is still an `Error::Api`, with `code == "unknown"` and whatever sentence it had in `message`.

| Status | Codes | Meaning |
|---|---|---|
| `400` | `invalid_payload` | Bad input; the message says what |
| `401` | `unauthorized` | Missing, wrong or revoked key; or a dead session |
| `402` | `ACTIVE_USER_LIMIT_REACHED`, `UPGRADE_REQUIRED`, `JOB_LIMIT_REACHED`, `PROJECT_LIMIT_REACHED`, `BILLING_HARD_BLOCKED`, `SUBSCRIPTION_PAST_DUE`, `payment_required` | A plan limit or a billing state; the numbers are in `details` |
| `403` | `forbidden`, `plan_required`, `mfa_required`, `account_disabled` | `forbidden` = the key lacks the scope for this API |
| `404` | `not_found` | Unknown id, or one that is not in your account |
| `409` | `conflict`, `limit_reached` | Already exists **in your account** (see below) |
| `429` | `rate_limited`, `RELAY_MESSAGE_LIMIT_REACHED` | See below |
| `500` | `internal_error` | |
| `502`/`503`/`504` | `upstream_unavailable` | Worth a retry |

`409` means *this already exists in your account*: a second end user with the same address in the same account, a role name already taken in that organization, a person already a member, a role still held by members, or the 100-roles cap. The same address in another account is a different person and is created.

**Retries.** A `POST` that gets a `429` is retried once, only when `Retry-After` is 60 seconds or less (absent counts as one second). `429 RELAY_MESSAGE_LIMIT_REACHED` — Relay's monthly limit, whose `Retry-After` runs to the 1st of next month — is returned at once, as is any longer wait; read `e.retry_after`. A `402` is never retried.

## Relay — webhooks

```rust
use nautilus_rs::SendMessageParams;

verne.relay()?.messages().send(SendMessageParams {
    event_type: "order.placed".into(),
    payload: serde_json::json!({ "order_id": "999" }),
    idempotency_key: Some("evt_abc".into()), // a repeat within 12 hours returns the first message
    channels: Some(vec!["team-a".into()]),   // optional: restrict to endpoint channels
}).await?;
```

List previously sent events, cursor-paged:

```rust
use nautilus_rs::ListMessagesParams;

let page = verne.relay()?.messages().list(ListMessagesParams {
    limit: Some(20),
    event_type: Some("order.placed".into()),
    cursor: None,
}).await?;
// page.data, page.has_more, page.next_cursor
```

## Gate — identities

An end user has two traits: `email` (required) and `locale`. **Application data does not go in traits** — the schema refuses anything else with `400`. Keep your own fields in custom metadata:

```rust
use nautilus_rs::{CreateIdentityParams, IdentityTraitsInput, JsonPatchOp};
use serde_json::json;

let gate = verne.gate()?;

let user = gate.identities().create(CreateIdentityParams {
    schema_id: "user".into(),
    traits: IdentityTraitsInput { email: "user@example.com".into(), locale: Some("en".into()) },
    credentials: Some(json!({ "password": { "config": { "password": "StrongPassword123!" } } })),
    state: None,
}).await?;

// Change the email (JSON Patch, RFC 6902)
gate.identities().patch(&user.id, vec![JsonPatchOp {
    op: "replace".into(),
    path: "/traits/email".into(),
    value: Some(json!("new@example.com")),
    from: None,
}]).await?;

// Custom metadata: merge a patch; a key set to null is removed.
gate.identities().update_metadata(&user.id, json!({ "plan": "pro", "seats": 5 })).await?;
let metadata = gate.identities().get_metadata(&user.id).await?; // serde_json::Map

// Deactivate: the user cannot sign in, sessions are suspended (not revoked),
// introspection answers active:false, no signed tokens are issued. Reversible.
gate.identities().deactivate(&user.id).await?;
gate.identities().activate(&user.id).await?;

gate.identities().delete(&user.id).await?;
```

`Identity` types the stable fields (`id`, `schema_id`, `state`, `traits`, `created_at`, `metadata_public`, …); the rest of the Kratos object is in `identity.extra`.

## Gate — organizations, roles and members

Organizations are the teams, workspaces or companies of your end users. Gate keeps the data and reports it (introspection, the session check, the signed token's `orgs`); it does not enforce permissions — ask with `can`.

```rust
use nautilus_rs::{AddMemberParams, CreateOrganizationParams, CreateRoleParams, ListParams, UpdateMemberParams};

let orgs = gate.organizations();

let org = orgs.create(CreateOrganizationParams { name: "Acme".into() }).await?;

let admin = orgs.roles().create(&org.id, CreateRoleParams {
    name: "admin".into(), // ^[a-z0-9][a-z0-9_-]{0,63}$, unique in the organization
    permissions: Some(vec!["invoices:read".into(), "invoices:write".into()]),
}).await?;

orgs.members().add(&org.id, AddMemberParams {
    identity_id: user.id.clone(),
    role_id: admin.id.clone(),
    expires_at: None, // or Some("2027-01-01T00:00:00Z".into())
}).await?;

// One page; loop on page.has_more with cursor: page.next_cursor
let page = orgs.list(ListParams { limit: Some(50), cursor: None }).await?;

// A user's organizations, with the role and permissions held in each
let held = gate.identities().list_organizations(&user.id, ListParams::default()).await?;
```

Changing a membership deadline tells three things apart with `expires_at: Option<Option<String>>`:

```rust
// None            -> keep the deadline (nothing is sent)
// Some(None)      -> remove it (sends null)
// Some(Some(..))  -> set it
orgs.members().update(&org.id, &user.id, UpdateMemberParams {
    role_id: None,
    expires_at: Some(None),
}).await?;
```

Permissions are lowercase segments joined by `:` (`invoices:write`); `*` and the `nautilus:` prefix are reserved. `roles().update` with `permissions` **replaces** the set. Deleting a role somebody holds is `409`; deleting an organization removes its roles and members.

### Checking a permission

```rust
if !gate.organizations().can(&org.id, &user.id, "invoices:write").await? {
    eprintln!("forbidden"); // answer 403 to your caller
}
```

`can` returns `true` only when the user is a member whose role holds the permission, and **returns errors instead of `false`**: an invalid permission is a `400`. For the reason, use `authorize`:

```rust
use nautilus_rs::AuthorizeReason;

let decision = gate.organizations().authorize(&org.id, &user.id, "invoices:write").await?;
match decision.reason {
    AuthorizeReason::MemberWithPermission => println!("yes, as {:?}", decision.role),
    AuthorizeReason::MemberWithoutPermission => println!("a member, but not allowed"),
    AuthorizeReason::NotAMember => println!("not in this organization"),
    AuthorizeReason::SubjectInactive => println!("the user is deactivated"),
    AuthorizeReason::NoSuchOrganization => println!("no such organization in this project"),
    other => println!("not allowed: {other}"), // the ownership reasons below, or a new one
}
```

`authorize` and `can` build `resource = "organization:<id>"` for you — nobody glues the string by hand. For a resource that is not an organization, `gate.authorize(AuthorizeParams { .. })` only answers *is the subject one of your active end users?* Its `reason` is a code too: `AuthorizeReason::OwnedByTenant` (the one `allowed: true`), `SubjectInactive`, `NotOwnedByTenant`, `NotAKnownIdentity`. The sentence a person would read is in `decision.message`, for your logs — its wording can change, `reason` is the contract. A reason the SDK does not know yet is `AuthorizeReason::Other(wire)`; treat it as a denial.

## Gate — sessions and the session check

```rust
// Gate your own routes on this: the session is alive and meets your MFA policy.
let check = gate.sessions().check(&session_token).await?;
if check.mfa_satisfied {
    // check.identity_id, check.orgs (the user's organizations, read now)
}
```

If your tenant requires a second factor and the session has none, `check` is `403 mfa_required` with `details["required_aal"] == "aal2"`. Branch on it and start a step-up flow:

```rust
use nautilus_rs::{Error, LoginFlowParams};

match gate.sessions().check(&session_token).await {
    Err(Error::Api(e)) if e.status == 403 && e.code == "mfa_required" => {
        let flow = gate.create_login_flow_with(LoginFlowParams {
            aal: Some("aal2".into()),
            session_token: Some(session_token.clone()), // sent as X-Session-Token
            ..Default::default()
        }).await?;
        // render flow["ui"] / post the code to flow["ui"]["action"]
    }
    Err(Error::Api(e)) if e.status == 401 => { /* dead session: sign in again */ }
    other => { other?; }
}

let sessions = gate.sessions().list(&user.id).await?;      // Vec<Session>
gate.sessions().revoke(&sessions[0].id).await?;            // idempotent
gate.sessions().revoke_all(&user.id).await?;               // everywhere
```

`revoke_all` is `Ok(())` also when the user has no session, so a "log out everywhere" button can be pressed twice. An id that is not one of your users (another account's, unknown, or one you deleted) is a `403 forbidden`.

## Gate — signed tokens (JWT)

For a platform that verifies signatures instead of calling Gate (Supabase, an API gateway), get a signed ES256 token for a signed-in user's session. The session token is the credential; no API key is sent.

```rust
let signed = gate.tokens().create_signed(&session_token).await?;
signed.token;   // the compact JWT
signed.session; // the session it was made from
```

Verify it offline (feature `jwt`) against your tenant's public keys, fetched once and cached; the issuer is `<base URL>/auth/<tenant id>`:

```rust
let claims = gate.tokens().verify_signed(&signed.token, &tenant_id).await?;
// claims.sub, claims.sid, claims.email, claims.orgs: Vec<OrgClaim>

// or, anywhere, with no API key and no Gate client:
use nautilus_rs::GateTokenVerifier;
let verifier = GateTokenVerifier::new(&tenant_id)?; // builder() for base_url, leeway
let claims = verifier.verify(&token).await?;
```

A token that must not be trusted is `Error::Token(e)` with `e.reason`: `Malformed`, `UnsupportedAlg` (anything but ES256, `none` included), `UnknownKid`, `BadSignature`, `WrongIssuer` (a token of another Gate tenant), `Expired`, `NotYetValid`, `MissingClaim`, `JwksUnavailable`. The verifier always checks `iss`.

The token lives **15 minutes** and **cannot be revoked**, and its `orgs` claim is as old as the token. For anything that must follow a revocation or a role change at once, use `sessions().check`, introspection or `organizations().can`.

The verifier uses `aws-lc-rs`, the crypto library `reqwest`'s TLS already builds, so the `jwt` feature adds no new crate to your build.

## Gate — access tokens

Exchange your key for a short-lived access token for a subject (a user or a service account), and validate one:

```rust
use nautilus_rs::CreateTokenParams;

let token = gate.tokens().create(CreateTokenParams {
    subject: user.id.clone(),
    scopes: None,
    ttl_seconds: Some(3600), // optional, default 3600, max 86400
}).await?;
// token.access_token (gat_live_…), token.expires_at, token.tenant_id

let info = gate.tokens().introspect(&token.access_token).await?;
if info.active {
    // info.subject, info.scopes, info.expires_at: Option<_>
    // info.orgs: the user's organizations, read now (info.orgs_truncated past 100)
}
```

An unknown, expired or foreign token — or one whose user you deactivated — is `{ active: false }` and nothing else: every other field is `None`. `Passepartout::introspect` has the same shape.

## Gate — audit log

```rust
use nautilus_rs::ListAuditLogParams;

let page = gate.audit_log().list(ListAuditLogParams {
    action: Some("organization.member_added".into()),
    target_id: Some(user.id.clone()),
    limit: Some(50),
    ..Default::default() // actor_id, actor_type, from, to (RFC 3339), cursor
}).await?;
for entry in &page.data {
    println!("{} {} by {}", entry.occurred_at, entry.action, entry.actor.kind);
}
```

Newest first, cursor-paged. **Any key with the `gate` scope reads the whole log, including your team members' IP addresses and user agents.** Keep `gate` keys on your backend.

## Gate — automatic deletion

Four rules per project — `DeleteUnverified`, `RemindUnverified`, `ExpireRoles`, `DeleteInactive` — all off until you switch them on. **Only `DeleteUnverified` is in the Free plan**; switching on another is `403 plan_required`.

```rust
use nautilus_rs::{LifecycleRuleKind, SetRuleParams};
use serde_json::json;

let rules = gate.lifecycle().list_rules().await?; // all four kinds, plus rules.tier

let preview = gate.lifecycle().preview_rule(
    LifecycleRuleKind::DeleteUnverified,
    json!({ "delete_after_days": 14, "warn_before_days": 3 }),
).await?; // preview.matching_now, preview.capped; writes nothing

gate.lifecycle().set_rule(LifecycleRuleKind::DeleteUnverified, SetRuleParams {
    enabled: true,
    params: Some(json!({ "delete_after_days": 14, "warn_before_days": 3 })), // None keeps the saved ones
}).await?;

let actions = gate.lifecycle().list_actions(Default::default()).await?; // what the rules scheduled and did
gate.lifecycle().delete_rule(LifecycleRuleKind::DeleteUnverified).await?;
```

## Gate — social login and settings

```rust
use nautilus_rs::{LoginFlowParams, SecuritySettings};

// Return URLs: where the user lands (with a one-time code) after a social login
gate.settings().update_return_urls(vec!["https://app.example.com/auth/callback".into()]).await?;
let flow = gate.create_login_flow_with(LoginFlowParams {
    return_to: Some("https://app.example.com/auth/callback".into()),
    state: Some("a-value-you-check-on-return".into()),
    ..Default::default()
}).await?;

// On the return URL: exchange the code (one use, about 60 seconds) for the session.
let result = gate.exchange_oidc_code(&code).await?; // result.session_token: keep it server-side

// Security settings (passwordless, TOTP MFA) — a full replacement, both fields are sent
gate.settings().update_security(SecuritySettings {
    passwordless_enabled: true,
    mfa_enabled: false,
}).await?;
```

Also: `settings().get_oidc_providers()` / `update_oidc_providers()`, `get_enabled_providers(tenant_id)`, `create_registration_flow(..)`, `identities().resend_verification(..)`.

## Passepartout — Telegram sign-in

```rust
let pp = verne.passepartout()?;
let flow = pp.login_start().await?;                 // flow.deep_link, flow.nonce
let status = pp.login_status(&flow.nonce).await?;   // poll until status.status is terminal
let info = pp.introspect("eyJ…").await?;            // info.active, info.orgs, …
```

## Clockwork — cron

Schedule recurring cron jobs and one-off delayed jobs that call your HTTP endpoints. List and executions endpoints return a plain `Vec<T>` (no pagination).

```rust
use nautilus_rs::{CreateCronJobParams, CreateDelayedJobParams, UpdateCronJobParams};

let clockwork = verne.clockwork()?;

let job = clockwork.jobs().create(CreateCronJobParams {
    name: "nightly-report".into(),
    schedule: "0 2 * * *".into(),
    url: "https://example.com/hooks/report".into(),
    ..Default::default() // method, headers, body
}).await?;

clockwork.jobs().update(&job.id, UpdateCronJobParams {
    is_active: Some(false),
    ..Default::default()
}).await?;

for exec in clockwork.jobs().executions(&job.id).await? {
    println!("attempt {:?}: {} {:?}", exec.attempt, exec.status, exec.response_status);
}
clockwork.jobs().delete(&job.id).await?;

clockwork.delayed().create(CreateDelayedJobParams {
    name: "send-reminder".into(),
    run_at: "2027-01-01T12:00:00Z".into(),
    url: "https://example.com/hooks/reminder".into(),
    ..Default::default()
}).await?;

// How many jobs exist against the plan's limits (limit: None = unlimited)
let usage = clockwork.usage().await?;
println!("{} cron jobs of {:?}", usage.cron.active, usage.cron.limit);
```

Creating a job over a plan limit is `402` (`UPGRADE_REQUIRED` / `JOB_LIMIT_REACHED`, never retried; the numbers are in `e.details`). A bad schedule is a `400` whose message is the engine's sentence, with `code == "unknown"`.

## Configuration

The builders accept a base URL and a timeout in seconds (default 30):

```rust
let verne = Verne::builder()
    .api_key(std::env::var("VERNE_API_KEY").unwrap())
    .base_url("http://api.soft.com") // a local stack; default https://api.vernesoft.com
    .timeout_secs(10)
    .build()?;
```

## Tests

`cargo test` runs the unit suites (mocked HTTP) and the doctests; `cargo test --all-features` adds the signed-token verifier, tested with locally generated P-256 keys.

`tests/integration_test.rs` is an opt-in suite against a running **local** stack, never part of `cargo test`: every case is `#[ignore]`. It refuses any host but `api.soft.com` and `localhost` unless `VERNE_ALLOW_REMOTE=1`, and needs a key with the scopes `gate`, `clockwork`, `passepartout`:

```bash
VERNE_API_KEY=vrn_… VERNE_BASE_URL=http://api.soft.com VERNE_EMAIL_PREFIX=e2e-sdk-mine \
  cargo test --features jwt --test integration_test -- --ignored --test-threads=1
```

`VERNE_EMAIL_PREFIX` names every end user it creates (`<prefix>-<n>@e2e.soft.com`); `VERNE_NARROW_KEY` (a key without the `gate` scope) additionally checks the `403` envelope. The Gate examples above (identities, organizations, sessions, signed tokens, audit log, automatic deletion, return URLs, metadata) and the Clockwork ones are calls this suite makes against the local stack; Relay sends and the Passepartout login need an engine set up and are covered by the mocked tests only.

## License

[MIT](LICENSE)
