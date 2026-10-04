//! Shared harness for the bluesky-claim-review-app acceptance suites
//! (DISTILL 2026-10-04).
//!
//! Included by each `review_app_*.rs` suite with
//! `#[path = "support/review_app/mod.rs"] mod review_app;` so the light suites
//! do not compile the 19k-line `support/mod.rs` (only the suites that also
//! drive the `openlore` CLI read path add `mod support;`).
//!
//! * [`domain`] — typed personas / philosophies / suggestions (Mandate-12 SSOT)
//! * [`app`]    — the REAL `openlore-review-app` process (third composition root)
//! * [`browser`] / [`html`] — a JS-less browser over the app's HTTP driving port
//! * [`world`]  — the hermetic world + the chained step vocabulary
//!
//! Driven-external systems are faked in `openlore-test-support`
//! (`FakeAtprotoNetwork`, `FakeGithubAccounts`); nothing internal is mocked.

#![allow(dead_code, unused_imports)]

pub mod app;
pub mod browser;
pub mod domain;
pub mod html;
pub mod world;

#[path = "../../../common/state_delta.rs"]
pub mod state_delta;

pub use app::{AppSettings, ReviewApp, Startup};
pub use browser::{Browser, Page};
pub use domain::*;
pub use world::*;
