//! Crafter-owned guard (step 01-04): the D-STZ-40 order at the walking skeleton.
//! WS-1 reads only the end state, so nothing else pins it until PA-11 (02-03).
#![cfg(unix)]

#[path = "../../../tests/common/state_delta.rs"]
mod state_delta;

#[path = "../../../tests/acceptance/support/indexer_lambda.rs"]
mod indexer_lambda;

use indexer_lambda::claims::*;
use indexer_lambda::*;

#[test]
fn the_pass_publishes_before_it_writes_its_summary() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    world.publishes(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
    let mark = world.timeline.mark();

    world.cold_pass_instance().schedule_fires();

    let at = |step| world.timeline.position_since(mark, step);
    let (publish, summary) = (at(step::PUBLISH), at(step::SUMMARY));
    assert!(
        publish.is_some() && publish < summary,
        "{:?}",
        world.timeline.since(mark)
    );
}
