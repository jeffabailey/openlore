//! DD-GDL-11 recorded-fixture contract: GitHub's real `/users/{login}` and
//! `/user/{id}` bodies (see `crates/test-support/fixtures/github/PROVENANCE.md`)
//! decode through the adapter's production reshape into the same account.

use adapter_github::client::parse_person_profile;

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
    let by_login = parse_person_profile(&recorded("users_profile.json"), "octocat");
    let by_id = parse_person_profile(&recorded("user_by_id.json"), "583231");

    assert_eq!(by_login.login, "octocat");
    assert_eq!(by_login.id, 583_231);
    assert_eq!(by_login.html_url, "https://github.com/octocat");
    assert_eq!(by_login.bio, None, "a null bio reads as no bio");
    assert_eq!(
        by_id, by_login,
        "/user/{{id}} serves the same profile shape"
    );
}
