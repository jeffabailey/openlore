//! `review-domain` — the PURE review/consent core of the hosted review app
//! (bluesky-claim-review-app, component-boundaries §1).
//!
//! No I/O, no async, no clock, no randomness: time and ids are passed in.
//! The composition root (`openlore-review-app`) decides nothing; every
//! business branch and every page body lives here.

#![forbid(unsafe_code)]

pub mod budget;
pub mod edits;
pub mod lifecycle;
pub mod ownership;
pub mod plans;
pub mod reconcile;
pub mod share;
pub mod signin;
pub mod views;
