use super::*;

fn parse_with(pairs: &[(&str, &str)], profile: BuildProfile) -> Result<IndexerConfig, ConfigError> {
    let env: Vec<(String, String)> = pairs
        .iter()
        .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
        .collect();
    parse_config(
        |name| {
            env.iter()
                .find(|(variable, _)| variable == name)
                .map(|(_, value)| value.clone())
        },
        profile,
    )
}

fn dids(list: &str) -> Result<IndexerConfig, ConfigError> {
    parse_with(&[(REPO_DIDS_VAR, list)], BuildProfile::Development)
}

fn problem_of(list: &str) -> String {
    dids(list).expect_err(list).problem
}

fn plc_did_of_length(length: usize) -> String {
    let prefix = "did:plc:";
    format!("{prefix}{}", "a".repeat(length - prefix.len()))
}

#[test]
fn a_did_of_the_maximum_length_loads_and_one_longer_is_malformed() {
    let longest = plc_did_of_length(MAX_DID_LENGTH);
    assert_eq!(dids(&longest).unwrap().repo_dids, vec![Did(longest)]);
    assert_eq!(
        problem_of(&plc_did_of_length(MAX_DID_LENGTH + 1)),
        "is not a well-formed DID"
    );
}

#[test]
fn each_malformed_did_shape_is_refused_as_malformed_not_as_an_unknown_method() {
    for entry in [
        "did::abc",
        "did:PLC:abc",
        "did:Plc:abc",
        "did:plc:",
        "did:plc:abc:",
        "did:plc:abc#frag",
        "did:plc:ab!c",
        "did:web:example.com/path",
    ] {
        assert_eq!(problem_of(entry), "is not a well-formed DID", "{entry}");
    }
}

#[test]
fn identifier_characters_of_the_did_syntax_are_accepted() {
    for entry in [
        "did:plc:abc123",
        "did:web:example.com",
        "did:web:example.com%3A8443",
        "did:web:example.com:user:alice",
        "did:plc:A_b-c.d",
    ] {
        assert_eq!(
            dids(entry).unwrap().repo_dids,
            vec![Did(entry.to_string())],
            "{entry}"
        );
    }
}

#[test]
fn a_well_formed_did_of_another_method_names_no_repo() {
    assert_eq!(
        problem_of("did:key:z6Mkabc"),
        "names no repo: only did:plc and did:web DIDs do"
    );
}

#[test]
fn a_refusal_displays_the_variable_its_quoted_value_and_the_problem() {
    let error = dids("did:key:z6Mkabc").unwrap_err();
    assert_eq!(
        error.to_string(),
        "OPENLORE_INDEXER_REPO_DIDS=\"did:key:z6Mkabc\": names no repo: only did:plc and did:web DIDs do"
    );
}

#[test]
fn the_default_index_path_lives_under_openlore_home() {
    let config = parse_with(&[(HOME_VAR, "/home/jeff")], BuildProfile::Development).unwrap();
    assert_eq!(
        config.index_path,
        PathBuf::from("/home/jeff/.local/share/openlore-indexer/index.duckdb")
    );
    let without_home = parse_with(&[], BuildProfile::Development).unwrap();
    assert_eq!(
        without_home.index_path,
        PathBuf::from("./.local/share/openlore-indexer/index.duckdb")
    );
}

fn fallback_under(policy_seam: bool, url: &str) -> Result<IndexerConfig, ConfigError> {
    let mut pairs = vec![(FALLBACK_VAR, url)];
    if policy_seam {
        pairs.push((LOOPBACK_SEAM_VAR, "1"));
    }
    parse_with(&pairs, BuildProfile::Development)
}

#[test]
fn the_test_seam_admits_a_plain_http_loopback_fallback() {
    let config = fallback_under(true, "http://127.0.0.1:8080/").unwrap();
    assert_eq!(config.policy, TransportPolicy::HttpsOrLoopbackHttp);
    assert_eq!(config.fallback.unwrap().as_str(), "http://127.0.0.1:8080");
    assert_eq!(
        fallback_under(false, "http://127.0.0.1:8080/")
            .unwrap_err()
            .variable,
        FALLBACK_VAR
    );
}

#[test]
fn the_test_seam_admits_no_https_fallback_to_a_refused_address() {
    for url in [
        "https://127.0.0.1:8443/",
        "https://[::1]:8443/",
        "https://10.0.0.1/",
    ] {
        for seam in [true, false] {
            let error = fallback_under(seam, url).unwrap_err();
            assert_eq!(error.variable, FALLBACK_VAR, "{url} seam={seam}");
            assert_eq!(error.value, url, "{url} seam={seam}");
        }
    }
}

#[test]
fn a_fallback_with_a_query_or_fragment_is_refused() {
    assert!(fallback_under(false, "https://relay.example.org/").is_ok());
    for url in [
        "https://relay.example.org/?cursor=1",
        "https://relay.example.org/#top",
        "http://127.0.0.1:8080/?x=1",
    ] {
        for seam in [true, false] {
            assert_eq!(
                fallback_under(seam, url).unwrap_err().variable,
                FALLBACK_VAR,
                "{url} seam={seam}"
            );
        }
    }
}
