//! 1.6.0: what the organization routes answer since Nautilus v1.10.44 — `member_count`,
//! a member's `email` and `display_name`, `total` on the lists, and the `reason` of a 404
//! or a 409.

mod common;

use common::{gate, JSON, ORG, ROLE, USER};
use nautilus_rs::{
    error_reason, AddMemberParams, ApiError, CreateRoleParams, Error, ListParams,
    UpdateMemberParams,
};

fn api(err: Error) -> ApiError {
    match err {
        Error::Api(e) => *e,
        other => panic!("expected Error::Api, got {other:?}"),
    }
}

fn refusal(code: &str, reason: Option<&str>) -> String {
    let reason = reason
        .map(|r| format!(r#","reason":"{r}""#))
        .unwrap_or_default();
    format!(r#"{{"error":{{"code":"{code}","message":"refused","request_id":"req_1"{reason}}}}}"#)
}

fn member_json(email: &str, display_name: &str) -> String {
    format!(
        r#"{{"organization_id":"{ORG}","identity_id":"{USER}","role_id":"{ROLE}","email":{email},"display_name":{display_name},"expires_at":null,"created_at":"2026-10-14T10:00:00Z","updated_at":"2026-10-14T10:00:00Z"}}"#
    )
}

#[tokio::test]
async fn organization_list_carries_total_and_each_organization_its_member_count() {
    let (mut server, gate) = gate().await;
    let list = server
        .mock("GET", "/v1/gate/organizations?limit=1")
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(format!(
            r#"{{"data":[{{"id":"{ORG}","name":"Acme","member_count":12,"created_at":"c","updated_at":"u"}}],"has_more":true,"next_cursor":"c2","total":37}}"#
        ))
        .create_async()
        .await;

    let page = gate
        .organizations()
        .list(ListParams {
            limit: Some(1),
            cursor: None,
        })
        .await
        .unwrap();

    assert_eq!(page.total, Some(37));
    assert_eq!(page.data[0].member_count, 12);
    list.assert_async().await;
}

#[tokio::test]
async fn role_list_carries_total_and_a_role_its_member_count() {
    let (mut server, gate) = gate().await;
    server
        .mock("GET", format!("/v1/gate/organizations/{ORG}/roles").as_str())
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(format!(
            r#"{{"data":[{{"id":"{ROLE}","organization_id":"{ORG}","name":"admin","permissions":[],"member_count":3,"created_at":"c","updated_at":"u"}}],"has_more":false,"next_cursor":null,"total":2}}"#
        ))
        .create_async()
        .await;

    let page = gate
        .organizations()
        .roles()
        .list(ORG, ListParams::default())
        .await
        .unwrap();

    assert_eq!(page.total, Some(2));
    assert_eq!(page.data[0].member_count, 3);
}

#[tokio::test]
async fn every_member_answer_carries_email_and_display_name() {
    let (mut server, gate) = gate().await;
    let base = format!("/v1/gate/organizations/{ORG}/members");
    server
        .mock("POST", base.as_str())
        .with_status(201)
        .with_header(JSON.0, JSON.1)
        .with_body(member_json(r#""alice@example.com""#, r#""Alice""#))
        .create_async()
        .await;
    // A Telegram user: no address, the first name they signed in with.
    server
        .mock("PATCH", format!("{base}/{USER}").as_str())
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(member_json("null", r#""Jules""#))
        .create_async()
        .await;
    server
        .mock("GET", base.as_str())
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(format!(
            r#"{{"data":[{}],"has_more":false,"next_cursor":null,"total":1}}"#,
            member_json(r#""alice@example.com""#, "null")
        ))
        .create_async()
        .await;

    let members = gate.organizations().members();
    let added = members
        .add(
            ORG,
            AddMemberParams {
                identity_id: USER.into(),
                role_id: ROLE.into(),
                expires_at: None,
            },
        )
        .await
        .unwrap();
    let updated = members
        .update(
            ORG,
            USER,
            UpdateMemberParams {
                role_id: Some(ROLE.into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let page = members.list(ORG, ListParams::default()).await.unwrap();

    assert_eq!(added.email.as_deref(), Some("alice@example.com"));
    assert_eq!(added.display_name.as_deref(), Some("Alice"));
    assert_eq!(updated.email, None);
    assert_eq!(updated.display_name.as_deref(), Some("Jules"));
    assert_eq!(page.total, Some(1));
    assert_eq!(page.data[0].email.as_deref(), Some("alice@example.com"));
    assert_eq!(page.data[0].display_name, None);
}

/// An answer without the new fields (a 1.5-era fixture, or a list that does not count)
/// still deserialises: counts are 0, the profile and `total` are `None`.
#[tokio::test]
async fn an_answer_without_the_new_fields_still_reads() {
    let (mut server, gate) = gate().await;
    server
        .mock("GET", format!("/v1/gate/organizations/{ORG}").as_str())
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(format!(
            r#"{{"id":"{ORG}","name":"Acme","created_at":"c","updated_at":"u"}}"#
        ))
        .create_async()
        .await;
    server
        .mock(
            "GET",
            format!("/v1/gate/identities/{USER}/organizations").as_str(),
        )
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(format!(
            r#"{{"data":[{{"id":"{ORG}","role":"admin","permissions":[]}}],"has_more":false,"next_cursor":null}}"#
        ))
        .create_async()
        .await;

    let org = gate.organizations().get(ORG).await.unwrap();
    let held = gate
        .identities()
        .list_organizations(USER, ListParams::default())
        .await
        .unwrap();

    assert_eq!(org.member_count, 0);
    assert_eq!(held.total, None);
}

#[tokio::test]
async fn organization_and_role_refusals_carry_their_reason() {
    let (mut server, gate) = gate().await;
    let org_path = format!("/v1/gate/organizations/{ORG}");
    let role_path = format!("{org_path}/roles/{ROLE}");
    server
        .mock("GET", org_path.as_str())
        .with_status(404)
        .with_header(JSON.0, JSON.1)
        .with_body(refusal("not_found", Some("organization")))
        .create_async()
        .await;
    server
        .mock("GET", role_path.as_str())
        .with_status(404)
        .with_header(JSON.0, JSON.1)
        .with_body(refusal("not_found", Some("role")))
        .create_async()
        .await;
    server
        .mock("DELETE", role_path.as_str())
        .with_status(409)
        .with_header(JSON.0, JSON.1)
        .with_body(refusal("conflict", Some("role_held")))
        .create_async()
        .await;
    let taken = server
        .mock("POST", format!("{org_path}/roles").as_str())
        .with_status(409)
        .with_header(JSON.0, JSON.1)
        .with_body(refusal("conflict", Some("role_name_taken")))
        .expect(1)
        .create_async()
        .await;

    let orgs = gate.organizations();
    let no_org = api(orgs.get(ORG).await.unwrap_err());
    let no_role = api(orgs.roles().get(ORG, ROLE).await.unwrap_err());
    let held = api(orgs.roles().delete(ORG, ROLE).await.unwrap_err());
    let name_taken = api(orgs
        .roles()
        .create(
            ORG,
            CreateRoleParams {
                name: "admin".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap_err());

    assert_eq!(
        (
            no_org.status,
            no_org.code.as_str(),
            no_org.reason.as_deref()
        ),
        (404, "not_found", Some(error_reason::ORGANIZATION))
    );
    assert_eq!(no_role.reason.as_deref(), Some(error_reason::ROLE));
    assert_eq!(
        (held.status, held.code.as_str(), held.reason.as_deref()),
        (409, "conflict", Some(error_reason::ROLE_HELD))
    );
    // Kept where 1.5 put it, so code reading details still works.
    assert_eq!(held.details["reason"], "role_held");
    assert_eq!(
        name_taken.reason.as_deref(),
        Some(error_reason::ROLE_NAME_TAKEN)
    );
    taken.assert_async().await;
}

#[tokio::test]
async fn member_refusals_carry_their_reason() {
    let (mut server, gate) = gate().await;
    let base = format!("/v1/gate/organizations/{ORG}/members");
    server
        .mock("POST", base.as_str())
        .with_status(404)
        .with_header(JSON.0, JSON.1)
        .with_body(refusal("not_found", Some("end_user")))
        .expect(1)
        .create_async()
        .await;
    server
        .mock("PATCH", format!("{base}/{USER}").as_str())
        .with_status(404)
        .with_header(JSON.0, JSON.1)
        .with_body(refusal("not_found", Some("member")))
        .create_async()
        .await;

    let members = gate.organizations().members();
    let stranger = api(members
        .add(
            ORG,
            AddMemberParams {
                identity_id: USER.into(),
                role_id: ROLE.into(),
                expires_at: None,
            },
        )
        .await
        .unwrap_err());
    let not_member = api(members
        .update(
            ORG,
            USER,
            UpdateMemberParams {
                role_id: Some(ROLE.into()),
                ..Default::default()
            },
        )
        .await
        .unwrap_err());

    assert_eq!(stranger.reason.as_deref(), Some(error_reason::END_USER));
    assert_eq!(not_member.reason.as_deref(), Some(error_reason::MEMBER));
}

#[tokio::test]
async fn already_member_and_a_role_limit_without_reason() {
    let (mut server, gate) = gate().await;
    let org_path = format!("/v1/gate/organizations/{ORG}");
    server
        .mock("POST", format!("{org_path}/members").as_str())
        .with_status(409)
        .with_header(JSON.0, JSON.1)
        .with_body(refusal("conflict", Some("already_member")))
        .create_async()
        .await;
    server
        .mock("POST", format!("{org_path}/roles").as_str())
        .with_status(409)
        .with_header(JSON.0, JSON.1)
        .with_body(refusal("limit_reached", None))
        .create_async()
        .await;

    let twice = api(gate
        .organizations()
        .members()
        .add(
            ORG,
            AddMemberParams {
                identity_id: USER.into(),
                role_id: ROLE.into(),
                expires_at: None,
            },
        )
        .await
        .unwrap_err());
    let limit = api(gate
        .organizations()
        .roles()
        .create(
            ORG,
            CreateRoleParams {
                name: "r101".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap_err());

    assert_eq!(twice.reason.as_deref(), Some(error_reason::ALREADY_MEMBER));
    assert_eq!(limit.code, "limit_reached");
    assert_eq!(limit.reason, None);
}

/// `reason` is read only from the envelope proper: next to a numeric code (an identity
/// service's body) it is a sentence, and a non-string `reason` is no reason.
#[tokio::test]
async fn reason_is_only_a_string_next_to_a_string_code() {
    let (mut server, gate) = gate().await;
    server
        .mock("DELETE", "/v1/gate/sessions/kratos")
        .with_status(400)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"error":{"code":400,"status":"Bad Request","reason":"The request was malformed.","message":"m"}}"#)
        .create_async()
        .await;
    server
        .mock("DELETE", "/v1/gate/sessions/numeric")
        .with_status(404)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"error":{"code":"not_found","message":"m","reason":42}}"#)
        .create_async()
        .await;
    server
        .mock("DELETE", "/v1/gate/sessions/new")
        .with_status(409)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"error":{"code":"conflict","message":"m","reason":"something_new"}}"#)
        .create_async()
        .await;

    let kratos = api(gate.sessions().revoke("kratos").await.unwrap_err());
    let numeric = api(gate.sessions().revoke("numeric").await.unwrap_err());
    let new = api(gate.sessions().revoke("new").await.unwrap_err());

    assert_eq!(kratos.code, "unknown");
    assert_eq!(kratos.reason, None);
    assert_eq!(numeric.reason, None);
    assert_eq!(new.reason.as_deref(), Some("something_new"));
}
