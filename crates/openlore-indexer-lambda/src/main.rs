//! `openlore-indexer-lambda <search|pass|probe>` — the Lambda entrypoint
//! (D-STZ-24: `image_config.entry_point` + `command`). The fourth composition
//! root's one effectful file: it reads the process's arguments and environment,
//! refuses what it cannot start with on a `health.startup.refused` line, builds
//! the adapters, and hands the initialised function to its event loop. Search
//! runs under `lambda_http`, the pass and the probe under `lambda_runtime`
//! (DESIGN § components). A function exists only once its `init` has passed its
//! probes, so nothing here can use one that has not (ADR-009).

use std::convert::Infallible;
use std::fmt::Display;
use std::io::Write;
use std::process::ExitCode;
use std::sync::Arc;

use adapter_s3_index_object::{S3IndexObjectAdapter, S3Location, SsmRepoDidListAdapter};
use adapter_system_clock::SystemClockAdapter;
use lambda_http::Request;
use lambda_runtime::LambdaEvent;
use openlore_indexer::config::BuildProfile;
use openlore_indexer_lambda::config::{parse_lambda_config, Mode, ModeSettings};
use openlore_indexer_lambda::{
    refuse_config, refuse_start, Environment, EventSink, InitRefused, PassDriven, PassFunction,
    ProbeDriven, ProbeFunction, SearchDriven, SearchFunction,
};
use ports::{ClockPort, ProbeRefused};
use serde_json::Value;
use tokio::runtime::Builder as RuntimeBuilder;

/// A refused start, as the self-host indexer exits on one.
const EXIT_REFUSED: u8 = 2;
/// The event loop itself gave up.
const EXIT_RUNTIME_FAILED: u8 = 1;

/// What a refused connect names (`health.startup.refused.adapter`).
const OBJECT_STORE_ADAPTER: &str = "index_object";
const DID_LIST_ADAPTER: &str = "repo_did_list";
const RUNTIME_ADAPTER: &str = "runtime";

/// Stdout, so the existing log metric filters match unchanged. A line that cannot
/// be written has nowhere else to go, and must not stop the function.
struct StdoutLines;

impl EventSink for StdoutLines {
    fn line(&self, line: &str) {
        let _ = writeln!(std::io::stdout().lock(), "{line}");
    }
}

/// One initialised function, ready for its event loop.
enum Function {
    Search(SearchFunction),
    Pass(PassFunction),
    Probe(ProbeFunction),
}

fn main() -> ExitCode {
    let events: Arc<dyn EventSink> = Arc::new(StdoutLines);
    let arguments: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    match RuntimeBuilder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime.block_on(run(&arguments, &process_environment(), events)),
        Err(failed) => {
            refuse_start(
                &events,
                RUNTIME_ADAPTER,
                ProbeRefused::index_object_unusable(format!("no async runtime: {failed}")),
            );
            ExitCode::from(EXIT_REFUSED)
        }
    }
}

/// The process's environment. A variable that is not text is no setting of ours.
fn process_environment() -> Environment {
    let readable = std::env::vars_os()
        .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)));
    Environment(readable.collect())
}

async fn run(arguments: &[String], env: &Environment, events: Arc<dyn EventSink>) -> ExitCode {
    let Ok(function) = start(arguments, env, events).await else {
        return ExitCode::from(EXIT_REFUSED);
    };
    match serve(function).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(failed) => {
            eprintln!("openlore-indexer-lambda: the event loop stopped: {failed}");
            ExitCode::from(EXIT_RUNTIME_FAILED)
        }
    }
}

/// Wire one function and let its `init` probe it. The key check comes first, so
/// `connect` never gets the chance to refuse without the event (D-STZ-39).
async fn start(
    arguments: &[String],
    env: &Environment,
    events: Arc<dyn EventSink>,
) -> Result<Function, InitRefused> {
    let mode = Mode::of_command(arguments).map_err(|refused| refuse_config(&events, refused))?;
    env.refuse_a_key_naming_the_marker(&events)?;
    let config = parse_lambda_config(env, mode, BuildProfile::of_this_build())
        .map_err(|refused| refuse_config(&events, refused))?;
    let store = connect_object_store(config.object_store, mode, &events).await?;
    let clock: Arc<dyn ClockPort + Send + Sync> = Arc::new(SystemClockAdapter::new());
    match config.settings {
        ModeSettings::Search(_) => {
            let driven = SearchDriven {
                index_object: store,
                clock,
                events,
            };
            SearchFunction::init(env, driven)
                .await
                .map(Function::Search)
        }
        ModeSettings::Pass { did_list } => {
            let repo_dids = SsmRepoDidListAdapter::connect(did_list)
                .await
                .map_err(|failed| refuse_connect(&events, DID_LIST_ADAPTER, &failed))?;
            let driven = PassDriven {
                index_read: Arc::clone(&store) as _,
                index_write: store,
                repo_dids: Arc::new(repo_dids),
                clock,
                events,
            };
            PassFunction::init(env, driven).await.map(Function::Pass)
        }
        ModeSettings::Probe(_) => {
            let driven = ProbeDriven {
                index_object: store,
                clock,
                events,
            };
            ProbeFunction::init(env, driven).await.map(Function::Probe)
        }
    }
}

/// The object store under the fetch budget `mode` runs on.
async fn connect_object_store(
    location: S3Location,
    mode: Mode,
    events: &Arc<dyn EventSink>,
) -> Result<Arc<S3IndexObjectAdapter>, InitRefused> {
    S3IndexObjectAdapter::connect(location)
        .await
        .map(|adapter| Arc::new(adapter.fetching_as(mode.fetch_call())))
        .map_err(|failed| refuse_connect(events, OBJECT_STORE_ADAPTER, &failed))
}

fn refuse_connect(
    events: &Arc<dyn EventSink>,
    adapter: &str,
    failed: &impl Display,
) -> InitRefused {
    let refused = ProbeRefused::index_object_unusable(failed.to_string());
    refuse_start(events, adapter, refused)
}

async fn serve(function: Function) -> Result<(), lambda_runtime::Error> {
    match function {
        Function::Search(search) => serve_search(search).await,
        Function::Pass(pass) => serve_pass(pass).await,
        Function::Probe(probe) => serve_probe(probe).await,
    }
}

async fn serve_search(search: SearchFunction) -> Result<(), lambda_runtime::Error> {
    let search = Arc::new(search);
    lambda_http::run(lambda_http::service_fn(move |request: Request| {
        let search = Arc::clone(&search);
        async move { Ok::<_, Infallible>(search.handle_request(request).await) }
    }))
    .await
}

async fn serve_pass(pass: PassFunction) -> Result<(), lambda_runtime::Error> {
    let pass = Arc::new(pass);
    lambda_runtime::run(lambda_runtime::service_fn(
        move |event: LambdaEvent<Value>| {
            let pass = Arc::clone(&pass);
            async move {
                let answer = pass.handle(event.payload).await;
                answer.map_err(|failed| failed.message)
            }
        },
    ))
    .await
}

async fn serve_probe(probe: ProbeFunction) -> Result<(), lambda_runtime::Error> {
    let probe = Arc::new(probe);
    lambda_runtime::run(lambda_runtime::service_fn(
        move |event: LambdaEvent<Value>| {
            let probe = Arc::clone(&probe);
            async move {
                let answer = probe.handle(event.payload).await;
                answer.map_err(|failed| failed.message)
            }
        },
    ))
    .await
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use adapter_s3_index_object::WRITE_PROBE_KEY;
    use openlore_indexer_lambda::config::{BUCKET_VAR, INDEX_KEY_VAR, LAST_PASS_KEY_VAR};

    use super::*;

    #[derive(Default)]
    struct RecordedLines(Mutex<Vec<String>>);

    impl EventSink for RecordedLines {
        fn line(&self, line: &str) {
            self.0.lock().expect("the lines").push(line.to_string());
        }
    }

    impl RecordedLines {
        /// The one line a refused start wrote.
        fn the_refusal(&self) -> Value {
            let lines = self.0.lock().expect("the lines");
            assert_eq!(lines.len(), 1, "{lines:?}");
            let line: Value = serde_json::from_str(&lines[0]).expect("a JSON line");
            assert_eq!(line["event"], "health.startup.refused", "{line}");
            assert_eq!(line["binary"], "openlore-indexer-lambda", "{line}");
            line
        }
    }

    fn environment(settings: &[(&str, &str)]) -> Environment {
        let named = settings.iter();
        Environment(
            named
                .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
                .collect(),
        )
    }

    fn a_start_with(arguments: &[&str], env: &Environment) -> (bool, Arc<RecordedLines>) {
        let lines = Arc::new(RecordedLines::default());
        let events: Arc<dyn EventSink> = Arc::clone(&lines) as _;
        let arguments: Vec<String> = arguments.iter().map(ToString::to_string).collect();
        let runtime = RuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .expect("a runtime");
        let started = runtime.block_on(start(&arguments, env, events));
        (started.is_ok(), lines)
    }

    // bypass: the parser's property covers every wrong command; this pins that a
    // refused one reaches the line, with no panic on the way.
    #[test]
    fn an_unknown_command_is_refused_on_the_startup_line() {
        let (started, lines) = a_start_with(&["serve"], &Environment::default());

        assert!(!started, "nothing starts");
        let line = lines.the_refusal();
        assert_eq!(line["structured"]["variable"], "command");
        assert_eq!(line["structured"]["value"], "serve");
    }

    // bypass: the parser's property covers every missing setting; this pins that
    // a refused one reaches the line before anything is connected.
    #[test]
    fn a_missing_setting_is_refused_on_the_startup_line() {
        let (started, lines) = a_start_with(&["search"], &Environment::default());

        assert!(!started, "nothing starts");
        assert_eq!(lines.the_refusal()["adapter"], "config");
    }

    // bypass: one fixed order: the event comes from the key check, which runs
    // before the bucket, or anything else, is even read.
    #[test]
    fn a_key_naming_the_marker_is_refused_before_anything_is_read() {
        for variable in [INDEX_KEY_VAR, LAST_PASS_KEY_VAR] {
            let env = environment(&[(variable, WRITE_PROBE_KEY)]);

            let (started, lines) = a_start_with(&["pass"], &env);

            assert!(!started, "{variable}");
            let line = lines.the_refusal();
            assert_eq!(line["reason"], "IndexerConfigInvalid", "{line}");
            assert!(
                line["detail"]
                    .as_str()
                    .expect("a detail")
                    .contains(WRITE_PROBE_KEY),
                "{line}"
            );
        }
    }

    // bypass: the one way `connect` fails, reached directly: `start` stops short
    // of it with the key check, so this proves the failure is a line and no panic.
    #[test]
    fn a_failed_connect_is_refused_on_the_startup_line() {
        let lines = Arc::new(RecordedLines::default());
        let events: Arc<dyn EventSink> = Arc::clone(&lines) as _;
        let location = S3Location {
            region: "us-east-1".to_string(),
            endpoint_url: None,
            bucket: BUCKET_VAR.to_string(),
            index_key: WRITE_PROBE_KEY.to_string(),
            last_pass_key: "index/last-pass.json".to_string(),
        };
        let runtime = RuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .expect("a runtime");

        let connected = runtime.block_on(connect_object_store(location, Mode::Search, &events));

        assert!(connected.is_err());
        assert_eq!(lines.the_refusal()["adapter"], OBJECT_STORE_ADAPTER);
    }
}
