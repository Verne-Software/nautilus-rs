use std::sync::Arc;

use reqwest::Method;

use super::types::{
    AddMemberParams, AuthorizationDecision, AuthorizeParams, CreateOrganizationParams,
    CreateRoleParams, Member, Organization, Role, UpdateMemberParams, UpdateOrganizationParams,
    UpdateRoleParams,
};
use crate::{
    error::Error,
    http::{urlencode as seg, Call, HttpClient},
    types::{ListParams, Paginated},
};

fn org_path(org_id: &str) -> String {
    format!("/v1/gate/organizations/{}", seg(org_id))
}

fn paged(path: impl Into<String>, params: ListParams) -> Call {
    Call::new(Method::GET, path)
        .query("limit", params.limit)
        .query("cursor", params.cursor)
}

/// Organizations of your end users — their teams, workspaces, companies — with named
/// roles and members. Everything here acts on **the project the API key belongs to**.
///
/// Gate keeps the data and reports it (introspection, the session check, the signed
/// token's `orgs` claim); it does not enforce permissions. Ask with [`can`](Self::can).
///
/// Obtain via [`Gate::organizations`](super::Gate::organizations).
pub struct OrganizationsClient {
    pub(super) http: Arc<HttpClient>,
}

impl OrganizationsClient {
    /// Return a [`RolesClient`] for the roles of an organization.
    pub fn roles(&self) -> RolesClient {
        RolesClient {
            http: Arc::clone(&self.http),
        }
    }

    /// Return a [`MembersClient`] for the members of an organization.
    pub fn members(&self) -> MembersClient {
        MembersClient {
            http: Arc::clone(&self.http),
        }
    }

    /// Create an organization. `name` is free text, 1–120 characters, not unique.
    ///
    /// Maps to `POST /v1/gate/organizations`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nautilus_rs::{Gate, CreateOrganizationParams};
    ///
    /// # async fn run() -> Result<(), nautilus_rs::Error> {
    /// let gate = Gate::new("vrn_9f8a7c1b2d3e4f50");
    /// let org = gate.organizations()
    ///     .create(CreateOrganizationParams { name: "Acme".into() })
    ///     .await?;
    /// println!("{}", org.id);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create(&self, params: CreateOrganizationParams) -> Result<Organization, Error> {
        self.http.post("/v1/gate/organizations", &params).await
    }

    /// List the project's organizations, oldest first, cursor-paged.
    ///
    /// Maps to `GET /v1/gate/organizations`.
    pub async fn list(&self, params: ListParams) -> Result<Paginated<Organization>, Error> {
        self.http
            .send(paged("/v1/gate/organizations", params))
            .await
    }

    /// Fetch one organization.
    ///
    /// Maps to `GET /v1/gate/organizations/{org_id}`.
    pub async fn get(&self, org_id: &str) -> Result<Organization, Error> {
        self.http.get(&org_path(org_id)).await
    }

    /// Rename an organization.
    ///
    /// Maps to `PATCH /v1/gate/organizations/{org_id}`.
    pub async fn update(
        &self,
        org_id: &str,
        params: UpdateOrganizationParams,
    ) -> Result<Organization, Error> {
        self.http.patch(&org_path(org_id), &params).await
    }

    /// Delete an organization with its roles and memberships (`204`).
    ///
    /// Maps to `DELETE /v1/gate/organizations/{org_id}`.
    pub async fn delete(&self, org_id: &str) -> Result<(), Error> {
        self.http.delete(&org_path(org_id)).await
    }

    /// *May this end user do this in this organization?* — answered from the role the
    /// user holds there and its permissions, as they are now.
    ///
    /// `permission` must match exactly (no wildcards). An organization of another project
    /// looks like a missing one ([`NoSuchOrganization`](crate::AuthorizeReason::NoSuchOrganization)).
    /// An invalid permission (upper case, `*`, the `nautilus:` prefix) is a `400` error,
    /// never `allowed: false`. Not billed, not audited.
    ///
    /// Builds `resource = "organization:<org_id>"` and calls
    /// `POST /v1/gate/authorize`, so nobody glues the string by hand.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nautilus_rs::{AuthorizeReason, Gate};
    ///
    /// # async fn run() -> Result<(), nautilus_rs::Error> {
    /// let gate = Gate::new("vrn_9f8a7c1b2d3e4f50");
    /// let decision = gate.organizations()
    ///     .authorize(
    ///         "550e8400-e29b-41d4-a716-446655440000",
    ///         "f47ac10b-58cc-4372-a567-0e02b2c3d479",
    ///         "invoices:write",
    ///     )
    ///     .await?;
    /// match decision.reason {
    ///     AuthorizeReason::MemberWithPermission => println!("yes, as {:?}", decision.role),
    ///     AuthorizeReason::MemberWithoutPermission => println!("a member, but not allowed"),
    ///     AuthorizeReason::NotAMember => println!("not in this organization"),
    ///     other => println!("no: {other}"),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn authorize(
        &self,
        org_id: &str,
        identity_id: &str,
        permission: &str,
    ) -> Result<AuthorizationDecision, Error> {
        self.http
            .post(
                "/v1/gate/authorize",
                &AuthorizeParams {
                    subject: identity_id.to_owned(),
                    action: permission.to_owned(),
                    resource: format!("organization:{org_id}"),
                    context: None,
                },
            )
            .await
    }

    /// `true` when the end user holds `permission` in the organization — the boolean of
    /// [`authorize`](Self::authorize). **Errors are returned, not turned into `false`**:
    /// a `400` for an invalid permission is an `Err`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nautilus_rs::Gate;
    ///
    /// # async fn run() -> Result<(), nautilus_rs::Error> {
    /// let gate = Gate::new("vrn_9f8a7c1b2d3e4f50");
    /// let (org_id, user_id) = ("550e8400-e29b-41d4-a716-446655440000", "f47ac10b-58cc-4372-a567-0e02b2c3d479");
    /// if !gate.organizations().can(org_id, user_id, "invoices:write").await? {
    ///     eprintln!("forbidden");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn can(
        &self,
        org_id: &str,
        identity_id: &str,
        permission: &str,
    ) -> Result<bool, Error> {
        Ok(self
            .authorize(org_id, identity_id, permission)
            .await?
            .allowed)
    }
}

/// Roles of one organization: named sets of permissions.
///
/// Obtain via [`OrganizationsClient::roles`].
pub struct RolesClient {
    http: Arc<HttpClient>,
}

impl RolesClient {
    fn path(org_id: &str) -> String {
        format!("{}/roles", org_path(org_id))
    }

    fn one(org_id: &str, role_id: &str) -> String {
        format!("{}/{}", Self::path(org_id), seg(role_id))
    }

    /// Define a role. `409 conflict` if the name is taken in the organization,
    /// `409 limit_reached` past 100 roles.
    ///
    /// Maps to `POST /v1/gate/organizations/{org_id}/roles`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nautilus_rs::{Gate, CreateRoleParams};
    ///
    /// # async fn run() -> Result<(), nautilus_rs::Error> {
    /// let gate = Gate::new("vrn_9f8a7c1b2d3e4f50");
    /// let role = gate.organizations().roles().create(
    ///     "550e8400-e29b-41d4-a716-446655440000",
    ///     CreateRoleParams {
    ///         name: "admin".into(),
    ///         permissions: Some(vec!["invoices:read".into(), "invoices:write".into()]),
    ///     },
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create(&self, org_id: &str, params: CreateRoleParams) -> Result<Role, Error> {
        self.http.post(&Self::path(org_id), &params).await
    }

    /// List an organization's roles, oldest first, cursor-paged.
    ///
    /// Maps to `GET /v1/gate/organizations/{org_id}/roles`.
    pub async fn list(&self, org_id: &str, params: ListParams) -> Result<Paginated<Role>, Error> {
        self.http.send(paged(Self::path(org_id), params)).await
    }

    /// Fetch one role.
    ///
    /// Maps to `GET /v1/gate/organizations/{org_id}/roles/{role_id}`.
    pub async fn get(&self, org_id: &str, role_id: &str) -> Result<Role, Error> {
        self.http.get(&Self::one(org_id, role_id)).await
    }

    /// Rename a role or **replace** its permission set.
    ///
    /// Maps to `PATCH /v1/gate/organizations/{org_id}/roles/{role_id}`.
    pub async fn update(
        &self,
        org_id: &str,
        role_id: &str,
        params: UpdateRoleParams,
    ) -> Result<Role, Error> {
        self.http.patch(&Self::one(org_id, role_id), &params).await
    }

    /// Delete a role. A role somebody still holds is `409 conflict` (the message has the
    /// count).
    ///
    /// Maps to `DELETE /v1/gate/organizations/{org_id}/roles/{role_id}`.
    pub async fn delete(&self, org_id: &str, role_id: &str) -> Result<(), Error> {
        self.http.delete(&Self::one(org_id, role_id)).await
    }
}

/// Members of one organization: end users and the role each holds.
///
/// Obtain via [`OrganizationsClient::members`].
pub struct MembersClient {
    http: Arc<HttpClient>,
}

impl MembersClient {
    fn path(org_id: &str) -> String {
        format!("{}/members", org_path(org_id))
    }

    fn one(org_id: &str, identity_id: &str) -> String {
        format!("{}/{}", Self::path(org_id), seg(identity_id))
    }

    /// Add an end user to the organization with a role, optionally until a deadline.
    ///
    /// `409 conflict` if they are already a member; `404` if the identity is not one of
    /// your end users. Maps to `POST /v1/gate/organizations/{org_id}/members`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nautilus_rs::{Gate, AddMemberParams};
    ///
    /// # async fn run() -> Result<(), nautilus_rs::Error> {
    /// let gate = Gate::new("vrn_9f8a7c1b2d3e4f50");
    /// let member = gate.organizations().members().add(
    ///     "550e8400-e29b-41d4-a716-446655440000",
    ///     AddMemberParams {
    ///         identity_id: "f47ac10b-58cc-4372-a567-0e02b2c3d479".into(),
    ///         role_id: "8e4f0b2a-3c1d-4e5f-9a6b-7c8d9e0f1a2b".into(),
    ///         expires_at: None,
    ///     },
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn add(&self, org_id: &str, params: AddMemberParams) -> Result<Member, Error> {
        self.http.post(&Self::path(org_id), &params).await
    }

    /// List an organization's members, oldest first, cursor-paged.
    ///
    /// Maps to `GET /v1/gate/organizations/{org_id}/members`.
    pub async fn list(&self, org_id: &str, params: ListParams) -> Result<Paginated<Member>, Error> {
        self.http.send(paged(Self::path(org_id), params)).await
    }

    /// Change a member's role and/or deadline. See [`UpdateMemberParams`] for how
    /// `expires_at` tells *keep*, *remove* and *set* apart.
    ///
    /// Maps to `PATCH /v1/gate/organizations/{org_id}/members/{identity_id}`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nautilus_rs::{Gate, UpdateMemberParams};
    ///
    /// # async fn run() -> Result<(), nautilus_rs::Error> {
    /// let gate = Gate::new("vrn_9f8a7c1b2d3e4f50");
    /// let (org, user) = ("550e8400-e29b-41d4-a716-446655440000", "f47ac10b-58cc-4372-a567-0e02b2c3d479");
    /// // set a deadline
    /// gate.organizations().members().update(org, user, UpdateMemberParams {
    ///     expires_at: Some(Some("2027-01-01T00:00:00Z".into())),
    ///     ..Default::default()
    /// }).await?;
    /// // remove it again
    /// gate.organizations().members().update(org, user, UpdateMemberParams {
    ///     expires_at: Some(None),
    ///     ..Default::default()
    /// }).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update(
        &self,
        org_id: &str,
        identity_id: &str,
        params: UpdateMemberParams,
    ) -> Result<Member, Error> {
        self.http
            .patch(&Self::one(org_id, identity_id), &params)
            .await
    }

    /// Remove a member from the organization.
    ///
    /// Maps to `DELETE /v1/gate/organizations/{org_id}/members/{identity_id}`.
    pub async fn remove(&self, org_id: &str, identity_id: &str) -> Result<(), Error> {
        self.http.delete(&Self::one(org_id, identity_id)).await
    }
}
