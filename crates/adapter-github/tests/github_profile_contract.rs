//! DD-GDL-11 recorded-fixture contract: GitHub's real `/users/{login}` and
//! `/user/{id}` bodies (see `crates/test-support/fixtures/github/PROVENANCE.md`)
//! decode through the adapter's production reshape into the same account,
//! and a body naming no account is an API-shape error (ADR-085 §4).

use adapter_github::client::parse_account_profile;

fn recorded(name: &str) -> serde_json::Value {
    let path = format!(
        "{}/../test-support/fixtures/github/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn the_login_read_and_the_account_number_read_decode_to_the_same_account() {
    let by_login = parse_account_profile(&recorded("users_profile.json")).expect("by login");
    let by_id = parse_account_profile(&recorded("user_by_id.json")).expect("by id");

    assert_eq!(by_login.login, "octocat");
    assert_eq!(by_login.id, 583_231);
    assert_eq!(by_login.html_url, "https://github.com/octocat");
    assert_eq!(by_login.bio, None, "a null bio reads as no bio");
    assert_eq!(
        by_id, by_login,
        "/user/{{id}} serves the same profile shape"
    );
}

/// The recorded profile with one field replaced (`None` removes it).
fn recorded_with(field: &str, value: Option<serde_json::Value>) -> serde_json::Value {
    let mut body = recorded("user_by_id.json");
    let object = body.as_object_mut().expect("a profile is a JSON object");
    match value {
        Some(value) => object.insert(field.to_string(), value),
        None => object.remove(field),
    };
    body
}

#[test]
fn a_profile_without_an_account_number_or_login_names_no_account() {
    for (case, body) in [
        ("no id", recorded_with("id", None)),
        ("id 0", recorded_with("id", Some(serde_json::json!(0)))),
        (
            "id as a string",
            recorded_with("id", Some(serde_json::json!("583231"))),
        ),
        (
            "negative id",
            recorded_with("id", Some(serde_json::json!(-1))),
        ),
        ("no login", recorded_with("login", None)),
        (
            "login not a string",
            recorded_with("login", Some(serde_json::json!(583_231))),
        ),
        (
            "blank login",
            recorded_with("login", Some(serde_json::json!("  "))),
        ),
    ] {
        let detail = parse_account_profile(&body).expect_err(case);
        let field = if case.contains("login") {
            "login"
        } else {
            "id"
        };
        assert!(detail.contains(field), "{case}: {detail}");
    }
}
