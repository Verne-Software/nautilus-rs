# Changelog

All notable changes to `nautilus-rs`. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## [1.6.0] — 2026-11-11

The first release on crates.io since 1.4.1: it also carries everything in 1.5.0 below.

What the organization routes answer since Nautilus v1.10.44, deserialised into the structs.

### Added

- **Who a member is**: `Member::email` and `Member::display_name` (`Option<String>`), on
  every answer that carries a member (`members().add()`, `list()`, `update()`). Both are
  `None` when the person has none: a Telegram user has no address, and their name is the
  first name (or the username) they signed in with.
- **Counts**: `Organization::member_count` (people holding a role in it) and
  `Role::member_count` (people holding that role — a role with members cannot be deleted),
  both `u64`. `Paginated::total` (`Option<u64>`): on `organizations().list()`,
  `roles().list()` and `members().list()` it is how many the whole list has, the same on
  every page; `None` on the lists that do not count.
- **Why a 404 or a 409**: `ApiError::reason` (`Option<String>`). On the organization
  routes a `404 not_found` names what was not found (`organization`, `role`, `member`,
  `end_user`, `project`) and a `409 conflict` why it is one (`role_name_taken`,
  `role_held`, `already_member`); the new `nautilus_rs::error_reason` module holds them as
  constants. `code` is unchanged, and the value stays in `details["reason"]` too, so 1.5
  code behaves as before. `reason` is read only from the API's envelope, never from an
  identity-service body passed through on `/auth/*`, where it is a sentence.
- The audit log records `organization.created` and `organization.updated` (a rename, with
  `previous_name` in `details`). `ListAuditLogParams::action` takes them as it takes any
  action; nothing changed in the SDK for that.

The new fields are public fields of response structs, as `CronJob::timezone` was in 1.5.0:
code that only reads them is unaffected; a struct literal of `Organization`, `Role`,
`Member`, `Paginated` or `ApiError` written by hand (in a test, say) needs the new fields.

## [1.5.0] — tagged 2026-10-25, not published

Tagged `v1.5.0` but never published to crates.io: its changes reach crates.io in 1.6.0. Going from 1.4.1 to 1.6.0 brings everything below.

### Added

- **Time zones for cron jobs**: `CronJob::timezone` (an IANA name, `"UTC"` unless set — also
  for an answer without the field), and `timezone` on `CreateCronJobParams` and
  `UpdateCronJobParams`. `"0 9 * * *"` in `"Europe/Paris"` runs at 09:00 Paris time all year
  round; `next_run_at` stays in UTC. An unknown zone is `400 invalid_payload`.
- **Importing users**: `CreateIdentityParams` gains `email_verified` (`Some(true)` creates the
  user with the address already confirmed) and `metadata_public`. `credentials` stays a
  `serde_json::Value`, so 1.4 code with `json!({…})` is unchanged; the new
  `IdentityCredentials` builds it typed — `IdentityCredentials::password(…)`,
  `::hashed_password(…)` (bcrypt), `.with_oidc_provider(provider, subject)` — and converts
  with `.into()` (over `PasswordCredential`, `PasswordConfig`, `OidcCredential`,
  `OidcConfig` and `OidcProviderLink`).
- `CreateIdentityParams` and `IdentityTraitsInput` derive `Default`. Ending a params literal
  in `..Default::default()` keeps it compiling when a later release adds an optional field
  (as this one does for `CreateIdentityParams`, `CreateCronJobParams` and
  `UpdateCronJobParams`).

## [1.4.1] — 2026-10-15

### Changed

- Package description and README opening name the product as Nautilus by Verne
  Software; registry metadata (homepage, documentation, keywords). No code changes.
  The crate documentation (docs.rs) names the product the same way.

## [1.4.0] — 2026-10-14

### Added

- **One key for every service**: `Verne::builder().api_key(…)`. Keys are `vrn_…` with
  scopes; the per-service setters (`relay`, `gate`, `passepartout`, `clockwork`) still work
  and override `api_key` for that service.
- **Organizations, roles and members**: `gate.organizations()` (`create`, `list`, `get`,
  `update`, `delete`), `.roles()` (`create`, `list`, `get`, `update`, `delete`),
  `.members()` (`add`, `list`, `update`, `remove`), and
  `gate.identities().list_organizations()`. `ListParams` for cursor paging.
- **Permission checks in an organization**: `organizations().authorize(org_id, identity_id,
  permission)` and `.can(…)` → `bool`. `AuthorizationDecision` gains `role`, and `reason` is
  the new `AuthorizeReason` enum (the organization reasons, the ownership reasons below,
  and `Other(String)` for a reason added later).
- **Audit log**: `gate.audit_log().list(ListAuditLogParams)` with filters and cursor paging.
- **Automatic deletion rules**: `gate.lifecycle()` (`list_rules`, `set_rule`, `delete_rule`,
  `preview_rule`, `list_actions`) and `LifecycleRuleKind`.
- **Signed tokens (JWT)**: `gate.tokens().create_signed(session_token)`; and, behind the new
  `jwt` cargo feature (off by default), `gate.tokens().verify_signed(token, tenant_id)` and
  the standalone `GateTokenVerifier` — ES256 against the tenant's JWKS, issuer checked,
  `kid`-driven refetch. Failures are `Error::Token(TokenError)` with a `TokenErrorReason`.
  The verifier uses `aws-lc-rs`, which `reqwest`'s TLS already builds: no new crate.
- **Sessions**: `gate.sessions()` (`list`, `revoke`, `revoke_all`, `check`). `check()` is
  `GET /v1/gate/auth/session`: `mfa_satisfied`, `orgs`, and `403 mfa_required`.
- **Custom metadata**: `identities().get_metadata()` / `update_metadata()`.
- **Social-login return**: `settings().get_return_urls()` / `update_return_urls()`,
  `gate.exchange_oidc_code()`, `gate.create_registration_flow()`, and
  `gate.create_login_flow_with(LoginFlowParams)` for `return_to`, `state`, `aal`,
  `session_token` (`create_login_flow()` stays).
- `orgs` / `orgs_truncated` on Gate and Passepartout introspection results.
- `clockwork.usage()`; `Execution::attempt`.
- `ApiError::details` (the extra members of the error: `plan`, `limit`, `required_aal`, …),
  `ApiError::retry_after` and `ApiError::body`.
- `Identity` types `created_at`, `updated_at`, `state_changed_at`, `metadata_public`,
  `verifiable_addresses` and `recovery_addresses`; the rest of the Kratos object is in
  `Identity::extra`.
- `tests/integration_test.rs`: an opt-in (`#[ignore]`) suite against a running local stack.
- Aligned with the API changes of October 2026: `AuthorizeReason` gains `OwnedByTenant`,
  `NotOwnedByTenant` and `NotAKnownIdentity` (with `SubjectInactive`, the codes of a check
  on a resource that is not an organization; `Other(String)` stays for a reason added
  later), and `AuthorizationDecision` gains `message`, the sentence that check used to put
  in `reason`. `sessions().revoke_all()` is documented as `Ok(())` for a user with no
  session (`204`, it was a `404`). Docs and comments name `invalid_payload` (not
  `invalid_request`) for a malformed token request and treat Clockwork errors as the
  standard envelope; the string-form and numeric-code parsing stays as a fallback.
  Identities no longer carry `metadata_public.verne_tenant_id`; the SDK never read it.

### Fixed

- `gate.tokens().create()` sent the key in the request body without an `Authorization`
  header, which the API answers with `401`. It now sends the header like every other call,
  and no longer puts the key in the body.
- A `429` was retried after waiting out any `Retry-After` — for Relay's monthly limit, up to
  a month. Only a `Retry-After` of 60 seconds or less is waited out now; the monthly limit
  (`RELAY_MESSAGE_LIMIT_REACHED`) is returned at once with `retry_after`, and a `402` is
  never retried.
- Error parsing no longer depends on the body being the documented envelope. A string
  `error` (Clockwork) was `Error::Http` and lost its message; a numeric `code` (an
  identity-service body) failed to parse; a non-JSON body was `Error::Http`. All are
  `Error::Api` now (`code == "unknown"` where the body had no string code).
- A response body that does not match the expected type is `Error::Json`, not `Error::Http`.
- Types: identity traits are `email` and `locale` — `custom_data` and `tenant_id` never
  existed in the API (creating with `custom_data` is a `400`); use custom metadata.
  Introspection fields other than `active` are optional (an inactive token is just
  `{"active": false}`).
- Ids in paths and values in query strings are percent-encoded (an opaque `cursor` could
  contain `+`, `/` or `=`).
- Docs: keys are `vrn_…` with no test/live environment; the Relay idempotency window is
  12 hours, not 24; `409` means "already exists in your account".

### Changed (source-incompatible; the reason is in *Fixed*)

- `Error` has a new variant, `Token(TokenError)`: an exhaustive `match` on `Error` needs an
  arm for it.
- `Error::Api` holds a `Box<ApiError>` (it grew past what `clippy::result_large_err`
  allows in every `Result`). `Error::Api(e)` patterns and `e.status`-style field access
  work as before; `*e` moves the `ApiError` out, and `From<ApiError> for Error` stays.
- `AuthorizationDecision::reason` is `AuthorizeReason`, not `String` (`Display` gives the
  wire string back).
- `TokenInfo::{subject, tenant_id, scopes, expires_at}` are `Option`.
- `IdentityTraitsInput` has `locale` instead of `custom_data`. `IdentityTraits` has
  `locale` and `extra` instead of `custom_data`; its `tenant_id` stays as a deprecated
  `Option`, always `None`.
- `ApiError` has three new public fields (`details`, `retry_after`, `body`).
  `ApiError::request_id` is an empty string when the response carried none (as before).
- `Identity` has the new fields listed above.

### Deprecated

- `IdentityTraits::tenant_id`: the API never returns it (nor, any longer,
  `metadata_public.verne_tenant_id`).
