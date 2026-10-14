# Changelog

All notable changes to `nautilus-rs`. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## [1.4.0] — unreleased

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
  the new `AuthorizeReason` enum (the five organization reasons, and `Other(String)` for
  the sentence of a non-organization check or a reason added later).
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

- `IdentityTraits::tenant_id`: the API never returns it.
