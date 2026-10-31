//! The one builder of client settings (DESIGN § Timeouts, UI-22): what every S3
//! and SSM call hands the SDK, derived from [`call_budget`] alone, so the SDK's
//! defaults never decide a worst case.
//!
//! What it overrides (`aws-sdk-s3` 1.157, behavior version `v2026_01_12`):
//!
//! * Retries default to standard mode with 3 attempts, and `AWS_MAX_ATTEMPTS` or
//!   `AWS_RETRY_MODE` can change that. An explicit setting wins over both.
//! * There is no default operation timeout, only a 3.1 s connect timeout. The
//!   operation timeout wraps the connect, so it is the bound on the calls below
//!   that figure.
//! * Stalled-stream protection defaults to on for uploads and downloads, with a
//!   5 s grace. That outlasts the shortest budgets, so the grace is capped by
//!   the call's own limit.
//!
//! A `GetObject` deserializes as a stream, so its operation timeout ends when
//! the headers arrive, and the body is read after `send` returns. For a download
//! the SDK bounds only a stall, not a slow body: whoever reads the body has to
//! hold it to the same budget.

use std::time::Duration;

use aws_config::retry::RetryConfig;
use aws_config::stalled_stream_protection::StalledStreamProtectionConfig;
use aws_config::timeout::TimeoutConfig;

use crate::{call_budget, CallBudget, StoreCall};

/// The SDK's own client default, kept as the upper bound on a stall.
const SDK_STALL_GRACE: Duration = Duration::from_secs(5);

/// What the SDK is told for one call. Apply it per operation with
/// `customize().config_override(...)`: the limits differ call by call.
#[derive(Debug, Clone)]
pub struct ClientSettings {
    pub retry: RetryConfig,
    pub timeouts: TimeoutConfig,
    pub stalled_stream: StalledStreamProtectionConfig,
}

/// The settings of one call, from the table.
pub fn client_settings(call: StoreCall) -> ClientSettings {
    settings_within(call_budget(call))
}

fn settings_within(budget: CallBudget) -> ClientSettings {
    ClientSettings {
        retry: RetryConfig::standard().with_max_attempts(budget.max_attempts),
        timeouts: TimeoutConfig::builder()
            .operation_timeout(budget.operation_timeout)
            .build(),
        stalled_stream: StalledStreamProtectionConfig::enabled()
            .grace_period(SDK_STALL_GRACE.min(budget.operation_timeout))
            .build(),
    }
}

impl ClientSettings {
    /// The S3 client's override for this call.
    pub fn s3_config(self) -> aws_sdk_s3::config::Builder {
        aws_sdk_s3::Config::builder()
            .retry_config(self.retry)
            .timeout_config(self.timeouts)
            .stalled_stream_protection(self.stalled_stream)
    }

    /// The SSM client's override for this call.
    pub fn ssm_config(self) -> aws_sdk_ssm::config::Builder {
        aws_sdk_ssm::Config::builder()
            .retry_config(self.retry)
            .timeout_config(self.timeouts)
            .stalled_stream_protection(self.stalled_stream)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_config::retry::RetryMode;

    // bypass: a closed enum of ten rows. Every row is checked, so a generated
    // input could only repeat them.
    const EVERY_CALL: [StoreCall; 10] = [
        StoreCall::WriteProbePut,
        StoreCall::ReadProbeHead,
        StoreCall::SearchInitGet,
        StoreCall::FreshnessHead,
        StoreCall::Reload,
        StoreCall::GetParameter,
        StoreCall::PassGet,
        StoreCall::Publish,
        StoreCall::PutLastPass,
        StoreCall::GetLastPass,
    ];

    /// What a client really holds, read back from the built SDK config.
    #[derive(Debug, PartialEq)]
    struct Held {
        attempts: u32,
        mode: RetryMode,
        limit: Option<Duration>,
        stall_grace: Duration,
        stall_watches_uploads: bool,
        stall_watches_downloads: bool,
    }

    fn held(
        retry: &RetryConfig,
        timeouts: &TimeoutConfig,
        stall: &StalledStreamProtectionConfig,
    ) -> Held {
        Held {
            attempts: retry.max_attempts(),
            mode: retry.mode(),
            limit: timeouts.operation_timeout(),
            stall_grace: stall.grace_period(),
            stall_watches_uploads: stall.upload_enabled(),
            stall_watches_downloads: stall.download_enabled(),
        }
    }

    fn s3_holds(call: StoreCall) -> Held {
        let config = client_settings(call).s3_config().build();
        held(
            config.retry_config().expect("retry set"),
            config.timeout_config().expect("timeouts set"),
            config.stalled_stream_protection().expect("stall set"),
        )
    }

    fn ssm_holds(call: StoreCall) -> Held {
        let config = client_settings(call).ssm_config().build();
        held(
            config.retry_config().expect("retry set"),
            config.timeout_config().expect("timeouts set"),
            config.stalled_stream_protection().expect("stall set"),
        )
    }

    fn expected(call: StoreCall, stall_grace: Duration) -> Held {
        Held {
            attempts: 1,
            mode: RetryMode::Standard,
            limit: Some(call_budget(call).operation_timeout),
            stall_grace,
            stall_watches_uploads: true,
            stall_watches_downloads: true,
        }
    }

    #[test]
    fn every_client_holds_one_attempt_the_tables_limit_and_a_stall_cap() {
        let stall_graces = [2, 2, 5, 1, 3, 5, 5, 5, 5, 3].map(Duration::from_secs);
        for (call, stall_grace) in EVERY_CALL.into_iter().zip(stall_graces) {
            assert_eq!(s3_holds(call), expected(call, stall_grace), "s3 {call:?}");
            assert_eq!(ssm_holds(call), expected(call, stall_grace), "ssm {call:?}");
        }
    }
}
