mod common;

use common::{gate, JSON, KEY, ORG, ROLE, USER};
use mockito::Matcher;
use nautilus_rs::{
    AddMemberParams, AuthorizeReason, CreateOrganizationParams, CreateRoleParams, Error,
    ListParams, UpdateMemberParams, UpdateOrganizationParams, UpdateRoleParams,
};
use serde_json::json;

fn org_json() -> String {
    format!(
        r#"{{"id":"{ORG}","name":"Acme","created_at":"2026-10-14T10:00:00Z","updated_at":"2026-10-14T10:00:00Z"}}"#
    )
}

fn role_json() -> String {
    format!(
        r#"{{"id":"{ROLE}","organization_id":"{ORG}","name":"admin","permissions":["invoices:read","invoices:write"],"created_at":"2026-10-14T10:00:00Z","updated_at":"2026-10-14T10:00:00Z"}}"#
    )
}

fn member_json(expires_at: &str) -> String {
    format!(
        r#"{{"organization_id":"{ORG}","identity_id":"{USER}","role_id":"{ROLE}","expires_at":{expires_at},"created_at":"2026-10-14T10:00:00Z","updated_at":"2026-10-14T10:00:00Z"}}"#
    )
}

#[tokio::test]
async fn create_organization_posts_the_name_with_the_key_in_the_header() {
    let (mut server, gate) = gate().await;
    let mock = server
        .mock("POST", "/v1/gate/organizations")
        .match_header("authorization", format!("Bearer {KEY}").as_str())
        .match_body(Matcher::Json(json!({"name": "Acme"})))
        .with_status(201)
        .with_header(JSON.0, JSON.1)
        .with_body(org_json())
        .create_async()
        .await;

    let org = gate
        .organizations()
        .create(CreateOrganizationParams {
            name: "Acme".into(),
        })
        .await
        .unwrap();

    assert_eq!(org.id, ORG);
    assert_eq!(org.name, "Acme");
    mock.assert_async().await;
}

#[tokio::test]
async fn list_organizations_sends_set_params_only_and_parses_the_page() {
    let (mut server, gate) = gate().await;
    // Exactly this query: a `cursor=` with no value would not match.
    let mock = server
        .mock("GET", "/v1/gate/organizations?limit=5")
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(format!(
            r#"{{"data":[{}],"has_more":true,"next_cursor":"c1+/="}}"#,
            org_json()
        ))
        .create_async()
        .await;
    let next = server
        .mock("GET", "/v1/gate/organizations?limit=5&cursor=c1%2B%2F%3D")
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"data":[],"has_more":false,"next_cursor":null}"#)
        .create_async()
        .await;

    let page = gate
        .organizations()
        .list(ListParams {
            limit: Some(5),
            cursor: None,
        })
        .await
        .unwrap();
    assert_eq!(page.data.len(), 1);
    assert!(page.has_more);

    // The opaque cursor is percent-encoded on the way out.
    let last = gate
        .organizations()
        .list(ListParams {
            limit: Some(5),
            cursor: page.next_cursor,
        })
        .await
        .unwrap();
    assert!(!last.has_more);
    assert_eq!(last.next_cursor, None);
    mock.assert_async().await;
    next.assert_async().await;
}

#[tokio::test]
async fn get_update_delete_organization() {
    let (mut server, gate) = gate().await;
    let get = server
        .mock("GET", format!("/v1/gate/organizations/{ORG}").as_str())
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(org_json())
        .create_async()
        .await;
    let patch = server
        .mock("PATCH", format!("/v1/gate/organizations/{ORG}").as_str())
        .match_body(Matcher::Json(json!({"name": "Acme Corp"})))
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(org_json().replace("Acme", "Acme Corp"))
        .create_async()
        .await;
    let delete = server
        .mock("DELETE", format!("/v1/gate/organizations/{ORG}").as_str())
        .with_status(204)
        .create_async()
        .await;

    let orgs = gate.organizations();
    assert_eq!(orgs.get(ORG).await.unwrap().name, "Acme");
    let renamed = orgs
        .update(
            ORG,
            UpdateOrganizationParams {
                name: "Acme Corp".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(renamed.name, "Acme Corp");
    orgs.delete(ORG).await.unwrap();

    get.assert_async().await;
    patch.assert_async().await;
    delete.assert_async().await;
}

#[tokio::test]
async fn ids_are_url_encoded_in_paths() {
    let (mut server, gate) = gate().await;
    let mock = server
        .mock("GET", "/v1/gate/organizations/a%2Fb%3Fc")
        .with_status(404)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"error":{"code":"not_found","message":"No such organization.","request_id":"req_1"}}"#)
        .create_async()
        .await;

    let err = gate.organizations().get("a/b?c").await.unwrap_err();
    assert!(matches!(err, Error::Api(e) if e.status == 404 && e.code == "not_found"));
    mock.assert_async().await;
}

#[tokio::test]
async fn roles_crud() {
    let (mut server, gate) = gate().await;
    let base = format!("/v1/gate/organizations/{ORG}/roles");
    let create = server
        .mock("POST", base.as_str())
        .match_body(Matcher::Json(
            json!({"name": "admin", "permissions": ["invoices:write", "invoices:read"]}),
        ))
        .with_status(201)
        .with_header(JSON.0, JSON.1)
        .with_body(role_json())
        .create_async()
        .await;
    let list = server
        .mock("GET", format!("{base}?limit=1").as_str())
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(format!(
            r#"{{"data":[{}],"has_more":false,"next_cursor":null}}"#,
            role_json()
        ))
        .create_async()
        .await;
    let get = server
        .mock("GET", format!("{base}/{ROLE}").as_str())
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(role_json())
        .create_async()
        .await;
    // PATCH sends only what is set: here the permission set, not the name.
    let update = server
        .mock("PATCH", format!("{base}/{ROLE}").as_str())
        .match_body(Matcher::Json(json!({"permissions": ["invoices:read"]})))
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(role_json())
        .create_async()
        .await;
    let delete = server
        .mock("DELETE", format!("{base}/{ROLE}").as_str())
        .with_status(409)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"error":{"code":"conflict","message":"Role is held by 1 member.","request_id":"req_2"}}"#)
        .create_async()
        .await;

    let roles = gate.organizations().roles();
    let role = roles
        .create(
            ORG,
            CreateRoleParams {
                name: "admin".into(),
                permissions: Some(vec!["invoices:write".into(), "invoices:read".into()]),
            },
        )
        .await
        .unwrap();
    // The API sorts and de-duplicates.
    assert_eq!(role.permissions, ["invoices:read", "invoices:write"]);
    assert_eq!(role.organization_id, ORG);

    let page = roles
        .list(
            ORG,
            ListParams {
                limit: Some(1),
                cursor: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(page.data[0].id, ROLE);
    assert_eq!(roles.get(ORG, ROLE).await.unwrap().name, "admin");
    roles
        .update(
            ORG,
            ROLE,
            UpdateRoleParams {
                permissions: Some(vec!["invoices:read".into()]),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let err = roles.delete(ORG, ROLE).await.unwrap_err();
    match err {
        Error::Api(e) => {
            assert_eq!((e.status, e.code.as_str()), (409, "conflict"));
            assert!(e.message.contains("held by 1 member"));
        }
        other => panic!("expected Error::Api, got {other:?}"),
    }

    create.assert_async().await;
    list.assert_async().await;
    get.assert_async().await;
    update.assert_async().await;
    delete.assert_async().await;
}

#[tokio::test]
async fn members_add_and_list() {
    let (mut server, gate) = gate().await;
    let base = format!("/v1/gate/organizations/{ORG}/members");
    let add = server
        .mock("POST", base.as_str())
        .match_body(Matcher::Json(
            json!({"identity_id": USER, "role_id": ROLE, "expires_at": "2027-01-01T00:00:00Z"}),
        ))
        .with_status(201)
        .with_header(JSON.0, JSON.1)
        .with_body(member_json(r#""2027-01-01T00:00:00Z""#))
        .create_async()
        .await;
    // Without a deadline `expires_at` is not sent at all.
    let add_open = server
        .mock("POST", base.as_str())
        .match_body(Matcher::Json(json!({"identity_id": USER, "role_id": ROLE})))
        .with_status(201)
        .with_header(JSON.0, JSON.1)
        .with_body(member_json("null"))
        .create_async()
        .await;
    let list = server
        .mock("GET", base.as_str())
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(format!(
            r#"{{"data":[{}],"has_more":false,"next_cursor":null}}"#,
            member_json("null")
        ))
        .create_async()
        .await;

    let members = gate.organizations().members();
    let dated = members
        .add(
            ORG,
            AddMemberParams {
                identity_id: USER.into(),
                role_id: ROLE.into(),
                expires_at: Some("2027-01-01T00:00:00Z".into()),
            },
        )
        .await
        .unwrap();
    assert_eq!(dated.expires_at.as_deref(), Some("2027-01-01T00:00:00Z"));

    let open = members
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
    assert_eq!(open.expires_at, None);

    let page = members.list(ORG, ListParams::default()).await.unwrap();
    assert_eq!(page.data[0].identity_id, USER);

    add.assert_async().await;
    add_open.assert_async().await;
    list.assert_async().await;
}

/// `expires_at` on a member update tells three things apart: absent keeps the deadline,
/// `null` removes it, a time sets it.
#[tokio::test]
async fn members_update_tells_absent_null_and_a_time_apart() {
    let (mut server, gate) = gate().await;
    let path = format!("/v1/gate/organizations/{ORG}/members/{USER}");
    let keep = server
        .mock("PATCH", path.as_str())
        .match_body(Matcher::Json(json!({"role_id": ROLE})))
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(member_json(r#""2027-01-01T00:00:00Z""#))
        .create_async()
        .await;
    let clear = server
        .mock("PATCH", path.as_str())
        .match_body(Matcher::Json(json!({"expires_at": null})))
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(member_json("null"))
        .create_async()
        .await;
    let set = server
        .mock("PATCH", path.as_str())
        .match_body(Matcher::Json(json!({"expires_at": "2027-06-01T00:00:00Z"})))
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(member_json(r#""2027-06-01T00:00:00Z""#))
        .create_async()
        .await;

    let members = gate.organizations().members();

    let kept = members
        .update(
            ORG,
            USER,
            UpdateMemberParams {
                role_id: Some(ROLE.into()),
                expires_at: None,
            },
        )
        .await
        .unwrap();
    assert!(kept.expires_at.is_some());

    let cleared = members
        .update(
            ORG,
            USER,
            UpdateMemberParams {
                role_id: None,
                expires_at: Some(None),
            },
        )
        .await
        .unwrap();
    assert_eq!(cleared.expires_at, None);

    let dated = members
        .update(
            ORG,
            USER,
            UpdateMemberParams {
                role_id: None,
                expires_at: Some(Some("2027-06-01T00:00:00Z".into())),
            },
        )
        .await
        .unwrap();
    assert_eq!(dated.expires_at.as_deref(), Some("2027-06-01T00:00:00Z"));

    keep.assert_async().await;
    clear.assert_async().await;
    set.assert_async().await;
}

#[tokio::test]
async fn members_remove() {
    let (mut server, gate) = gate().await;
    let mock = server
        .mock(
            "DELETE",
            format!("/v1/gate/organizations/{ORG}/members/{USER}").as_str(),
        )
        .with_status(204)
        .create_async()
        .await;

    gate.organizations()
        .members()
        .remove(ORG, USER)
        .await
        .unwrap();
    mock.assert_async().await;
}

#[tokio::test]
async fn identities_list_organizations() {
    let (mut server, gate) = gate().await;
    let mock = server
        .mock(
            "GET",
            format!("/v1/gate/identities/{USER}/organizations?limit=20").as_str(),
        )
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(format!(
            r#"{{"data":[{{"id":"{ORG}","role":"admin","permissions":["invoices:read","invoices:write"]}}],"has_more":false,"next_cursor":null}}"#
        ))
        .create_async()
        .await;

    let page = gate
        .identities()
        .list_organizations(
            USER,
            ListParams {
                limit: Some(20),
                cursor: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(page.data.len(), 1);
    assert_eq!(page.data[0].role, "admin");
    assert_eq!(
        page.data[0].permissions,
        ["invoices:read", "invoices:write"]
    );
    mock.assert_async().await;
}

#[tokio::test]
async fn authorize_builds_the_organization_resource() {
    let (mut server, gate) = gate().await;
    let mock = server
        .mock("POST", "/v1/gate/authorize")
        .match_body(Matcher::Json(json!({
            "subject": USER,
            "action": "invoices:write",
            "resource": format!("organization:{ORG}"),
        })))
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"allowed":true,"decision_id":"dec_1","reason":"member_with_permission","role":"admin"}"#)
        .create_async()
        .await;

    let decision = gate
        .organizations()
        .authorize(ORG, USER, "invoices:write")
        .await
        .unwrap();

    assert!(decision.allowed);
    assert_eq!(decision.reason, AuthorizeReason::MemberWithPermission);
    assert_eq!(decision.role.as_deref(), Some("admin"));
    mock.assert_async().await;
}

#[tokio::test]
async fn every_organization_reason_parses_and_displays_as_its_wire_string() {
    let cases = [
        (
            "member_with_permission",
            AuthorizeReason::MemberWithPermission,
            true,
            Some("admin"),
        ),
        (
            "member_without_permission",
            AuthorizeReason::MemberWithoutPermission,
            true,
            Some("viewer"),
        ),
        ("not_a_member", AuthorizeReason::NotAMember, false, None),
        (
            "subject_inactive",
            AuthorizeReason::SubjectInactive,
            true,
            Some("admin"),
        ),
        (
            "no_such_organization",
            AuthorizeReason::NoSuchOrganization,
            false,
            None,
        ),
    ];
    for (wire, expected, _, role) in cases {
        let body = match role {
            Some(role) => {
                format!(
                    r#"{{"allowed":false,"decision_id":"d","reason":"{wire}","role":"{role}"}}"#
                )
            }
            None => format!(r#"{{"allowed":false,"decision_id":"d","reason":"{wire}"}}"#),
        };
        let decision: nautilus_rs::AuthorizationDecision = serde_json::from_str(&body).unwrap();
        assert_eq!(decision.reason, expected);
        assert_eq!(decision.reason.to_string(), wire);
        assert_eq!(decision.role.as_deref(), role);
    }
}

#[tokio::test]
async fn an_unknown_reason_is_other_and_round_trips() {
    let decision: nautilus_rs::AuthorizationDecision = serde_json::from_str(
        r#"{"allowed":true,"decision_id":"d","reason":"subject `x` is an identity owned by tenant `y`"}"#,
    )
    .unwrap();
    assert_eq!(
        decision.reason,
        AuthorizeReason::Other("subject `x` is an identity owned by tenant `y`".into())
    );
    assert_eq!(
        decision.reason.to_string(),
        "subject `x` is an identity owned by tenant `y`"
    );
    assert_eq!(
        serde_json::to_value(&decision.reason).unwrap(),
        json!("subject `x` is an identity owned by tenant `y`")
    );
}

#[tokio::test]
async fn can_returns_the_boolean() {
    let (mut server, gate) = gate().await;
    server
        .mock("POST", "/v1/gate/authorize")
        .match_body(Matcher::PartialJson(json!({"action": "invoices:write"})))
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"allowed":true,"decision_id":"d","reason":"member_with_permission","role":"admin"}"#)
        .create_async()
        .await;
    server
        .mock("POST", "/v1/gate/authorize")
        .match_body(Matcher::PartialJson(json!({"action": "members:invite"})))
        .with_status(200)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"allowed":false,"decision_id":"d","reason":"member_without_permission","role":"admin"}"#)
        .create_async()
        .await;

    let orgs = gate.organizations();
    assert!(orgs.can(ORG, USER, "invoices:write").await.unwrap());
    assert!(!orgs.can(ORG, USER, "members:invite").await.unwrap());
}

/// An invalid permission is a `400`, never `false`.
#[tokio::test]
async fn can_propagates_errors_instead_of_answering_false() {
    let (mut server, gate) = gate().await;
    server
        .mock("POST", "/v1/gate/authorize")
        .with_status(400)
        .with_header(JSON.0, JSON.1)
        .with_body(r#"{"error":{"code":"invalid_payload","message":"Invalid permission.","request_id":"req_3"}}"#)
        .create_async()
        .await;

    let err = gate
        .organizations()
        .can(ORG, USER, "Invoices:*")
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Api(e) if e.status == 400 && e.code == "invalid_payload"));
}
