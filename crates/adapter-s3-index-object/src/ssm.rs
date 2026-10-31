//! The SSM call behind the operator's DID list (D-STZ-16).
//!
//! The text comes back exactly as saved. Whitespace, separators and bad DIDs are
//! the ADR-081 loader's to read, so nothing here looks inside it. The list is a
//! Standard String with no KMS key, so the call asks for no decryption.
//!
//! `GetParameter` is not a stream, so its operation timeout bounds the whole call
//! (DESIGN § Timeouts). The start-up probe is the same read under the same bound:
//! the table gives SSM one row.

use aws_sdk_ssm::error::DisplayErrorContext;
use aws_sdk_ssm::Client;
use ports::{ProbeOutcome, ProbeRefusalReason, ProbeRefused, RepoDidListError};
use serde_json::json;

use crate::client_settings::client_settings;
use crate::{shared_config, SsmLocation, StoreCall};

fn unavailable(detail: String) -> RepoDidListError {
    RepoDidListError::Unavailable { detail }
}

/// The list as the store holds it. An answer with no value is not a list.
fn saved_text(value: Option<&str>) -> Result<String, RepoDidListError> {
    value
        .map(str::to_string)
        .ok_or_else(|| unavailable("the parameter store gave no value".to_string()))
}

/// Having the list is what the probe proves. Whether it is a good list is the
/// loader's question, so any text at all passes.
fn list_probe_outcome(parameter: &str, read: Result<String, RepoDidListError>) -> ProbeOutcome {
    match read {
        Ok(_) => ProbeOutcome::Ok,
        Err(failed) => ProbeRefused {
            reason: ProbeRefusalReason::IndexerConfigInvalid,
            detail: failed.to_string(),
            structured: json!({ "parameter": parameter }),
        }
        .into(),
    }
}

/// The shared client (see `shared_config`), at the configured endpoint.
async fn client_at(location: &SsmLocation) -> Client {
    let shared = shared_config(&location.region).await;
    let config = aws_sdk_ssm::config::Builder::from(&shared);
    let config = match &location.endpoint_url {
        Some(endpoint) => config.endpoint_url(endpoint),
        None => config,
    };
    Client::from_conf(config.build())
}

/// The one parameter that holds the list.
pub(crate) struct DidListParameter {
    client: Client,
    parameter: String,
}

impl DidListParameter {
    pub(crate) async fn connect(location: SsmLocation) -> Self {
        Self {
            client: client_at(&location).await,
            parameter: location.parameter,
        }
    }

    pub(crate) async fn read(&self) -> Result<String, RepoDidListError> {
        let answer = self
            .client
            .get_parameter()
            .name(&self.parameter)
            .customize()
            .config_override(client_settings(StoreCall::GetParameter).ssm_config())
            .send()
            .await
            .map_err(|failed| unavailable(DisplayErrorContext(failed).to_string()))?;
        saved_text(answer.parameter().and_then(|held| held.value()))
    }

    pub(crate) async fn probe(&self) -> ProbeOutcome {
        list_probe_outcome(&self.parameter, self.read().await)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn any_failure() -> impl Strategy<Value = RepoDidListError> {
        "[ -~]{0,20}".prop_flat_map(|detail| {
            prop_oneof![
                Just(RepoDidListError::Unavailable {
                    detail: detail.clone()
                }),
                Just(RepoDidListError::ProbeRefused { detail }),
            ]
        })
    }

    proptest! {
        // RIO-4, KPI-IXD-4: the loader sees what the operator saved, whitespace
        // and separators included.
        #[test]
        fn the_list_comes_back_exactly_as_saved(
            text in "[ \t\r\n]{0,3}[ -~\t\r\n]{0,40}[ \t\r\n]{0,3}",
        ) {
            prop_assert_eq!(saved_text(Some(&text)), Ok(text));
        }

        // The loader judges the list, so the probe passes whatever it holds.
        #[test]
        fn any_list_that_can_be_read_passes_the_probe(text in "[ -~\n]{0,40}") {
            let outcome = list_probe_outcome("/p", Ok(text));

            prop_assert!(matches!(outcome, ProbeOutcome::Ok), "{:?}", outcome);
        }

        // RIO-5: a list that cannot be read refuses, and the refusal says which.
        #[test]
        fn a_list_that_cannot_be_read_refuses_and_names_the_parameter(
            parameter in "/[a-z/-]{1,20}",
            failed in any_failure(),
        ) {
            let outcome = list_probe_outcome(&parameter, Err(failed.clone()));

            let ProbeOutcome::Refused { reason, detail, structured } = outcome else {
                panic!("a failed read is a refusal")
            };
            prop_assert_eq!(reason, ProbeRefusalReason::IndexerConfigInvalid);
            prop_assert_eq!(detail, failed.to_string());
            prop_assert_eq!(structured, json!({ "parameter": parameter }));
        }
    }

    // bypass: the one absence a value can have.
    #[test]
    fn an_answer_with_no_value_is_not_a_list() {
        assert!(matches!(
            saved_text(None),
            Err(RepoDidListError::Unavailable { .. })
        ));
    }

    // bypass: a fixed fact of one built client, read back from the SDK config: no
    // default HTTPS client exists here, and a call with no override cannot retry.
    #[tokio::test]
    async fn the_shared_client_has_an_http_client_and_one_attempt() {
        let client = client_at(&SsmLocation {
            region: "us-east-1".to_string(),
            endpoint_url: None,
            parameter: "/p".to_string(),
        })
        .await;

        let config = client.config();
        assert!(config.http_client().is_some());
        assert_eq!(config.retry_config().expect("retry").max_attempts(), 1);
    }
}
