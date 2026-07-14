// Tests for FamilyBlockMoveSelector.

use std::any::TypeId;

use crate::heuristic::r#move::Move;
use crate::heuristic::selector::entity::FromSolutionEntitySelector;
use crate::heuristic::selector::family_block::FamilyBlockMoveSelector;
use crate::heuristic::selector::MoveSelector;
use solverforge_core::domain::{
    EntityCollectionExtractor, EntityDescriptor, PlanningSolution, SolutionDescriptor,
};
use solverforge_core::score::SoftScore;
use solverforge_scoring::ScoreDirector;

#[derive(Clone, Debug)]
struct Vehicle {
    visits: Vec<i32>,
}

#[derive(Clone, Debug)]
struct Plan {
    vehicles: Vec<Vehicle>,
    score: Option<SoftScore>,
}

impl PlanningSolution for Plan {
    type Score = SoftScore;

    fn score(&self) -> Option<Self::Score> {
        self.score
    }

    fn set_score(&mut self, score: Option<Self::Score>) {
        self.score = score;
    }
}

fn get_vehicles(plan: &Plan) -> &Vec<Vehicle> {
    &plan.vehicles
}

fn get_vehicles_mut(plan: &mut Plan) -> &mut Vec<Vehicle> {
    &mut plan.vehicles
}

fn descriptor() -> SolutionDescriptor {
    let extractor = Box::new(EntityCollectionExtractor::new(
        "Vehicle",
        "vehicles",
        get_vehicles,
        get_vehicles_mut,
    ));
    let entity_desc = EntityDescriptor::new("Vehicle", TypeId::of::<Vehicle>(), "vehicles")
        .with_extractor(extractor);
    SolutionDescriptor::new("Plan", TypeId::of::<Plan>()).with_entity(entity_desc)
}

fn create_director(vehicles: Vec<Vehicle>) -> ScoreDirector<Plan, ()> {
    let solution = Plan {
        vehicles,
        score: None,
    };
    ScoreDirector::simple(solution, descriptor(), |plan, _| plan.vehicles.len())
}

fn list_len(plan: &Plan, entity_idx: usize) -> usize {
    plan.vehicles
        .get(entity_idx)
        .map_or(0, |vehicle| vehicle.visits.len())
}
fn list_get(plan: &Plan, entity_idx: usize, pos: usize) -> Option<i32> {
    plan.vehicles
        .get(entity_idx)
        .and_then(|vehicle| vehicle.visits.get(pos))
        .copied()
}

fn sublist_remove(plan: &mut Plan, entity_idx: usize, start: usize, end: usize) -> Vec<i32> {
    plan.vehicles
        .get_mut(entity_idx)
        .map(|vehicle| vehicle.visits.drain(start..end).collect())
        .unwrap_or_default()
}

fn sublist_insert(plan: &mut Plan, entity_idx: usize, pos: usize, items: Vec<i32>) {
    if let Some(vehicle) = plan.vehicles.get_mut(entity_idx) {
        for (offset, item) in items.into_iter().enumerate() {
            vehicle.visits.insert(pos + offset, item);
        }
    }
}

// Family key = value's tens-digit-free identity: values that should be considered
// the same "family" share the same key. -1 (and any negative) has no family.
fn family_key(_plan: &Plan, v: i32) -> Option<u64> {
    if v < 0 {
        None
    } else {
        Some((v / 100) as u64)
    }
}

// Every op is eligible for any owner (empty eligible set).
fn eligible_any(_plan: &Plan, _v: i32) -> Vec<usize> {
    Vec::new()
}

// Values >= 900 are only eligible for owner 0.
fn eligible_owner_zero_only_for_900s(_plan: &Plan, v: i32) -> Vec<usize> {
    if v >= 900 { vec![0] } else { Vec::new() }
}

fn family_block_selector(
    min_block_size: usize,
    eligible_fn: fn(&Plan, i32) -> Vec<usize>,
) -> FamilyBlockMoveSelector<Plan, i32, FromSolutionEntitySelector> {
    FamilyBlockMoveSelector::<Plan, i32, _>::new(
        FromSolutionEntitySelector::new(0),
        min_block_size,
        list_len,
        list_get,
        sublist_remove,
        sublist_insert,
        family_key,
        eligible_fn,
        "visits",
        0,
    )
}

#[test]
fn blocks_in_only_emits_maximal_runs_at_least_min_block_size() {
    // [A, A, B] with family(A) = family(B/100) — use family key = v/100 so that
    // 100,101 share family 1 and 200 is family 2.
    let director = create_director(vec![Vehicle {
        visits: vec![100, 101, 200],
    }]);

    let selector = family_block_selector(2, eligible_any);
    let moves: Vec<_> = selector
        .iter_moves(&director)
        .map(|m| (m.source_entity_index(), m.source_start(), m.source_end()))
        .collect();

    // Only the (0,2) same-family block should ever be a *source* block; the
    // trailing single "200" must never appear as a source range on its own.
    assert!(
        moves
            .iter()
            .all(|&(_, start, end)| (start, end) == (0, 2)),
        "single-op runs must never be emitted as a source block: {moves:?}"
    );
    assert!(!moves.is_empty(), "the (0,2) block should still relocate somewhere");
}

#[test]
fn insert_pos_lands_immediately_after_same_family_op() {
    // Source vehicle 0 has the block [100, 101] (family 1). Destination vehicle 1
    // already has a family-1 op (150) at position 1, so the block must land at
    // position 2 (immediately after it) — the *lowest* such position.
    let director = create_director(vec![
        Vehicle {
            visits: vec![100, 101, 999],
        },
        Vehicle {
            visits: vec![50, 150, 250],
        },
    ]);

    let selector = family_block_selector(2, eligible_any);
    let moves: Vec<_> = selector
        .iter_moves(&director)
        .filter(|m| m.source_entity_index() == 0 && m.dest_entity_index() == 1)
        .map(|m| m.dest_position())
        .collect();

    assert_eq!(
        moves,
        vec![2],
        "block must land immediately after the lowest same-family-adjacent op"
    );
}

#[test]
fn insert_pos_appends_at_end_when_no_same_family_op_on_destination() {
    // Destination vehicle 1 has no family-1 op at all, so the block must append
    // at the destination's (post-removal-irrelevant, since inter-entity) end.
    let director = create_director(vec![
        Vehicle {
            visits: vec![100, 101, 999],
        },
        Vehicle {
            visits: vec![250, 350],
        },
    ]);

    let selector = family_block_selector(2, eligible_any);
    let moves: Vec<_> = selector
        .iter_moves(&director)
        .filter(|m| m.source_entity_index() == 0 && m.dest_entity_index() == 1)
        .map(|m| m.dest_position())
        .collect();

    assert_eq!(moves, vec![2], "block must append at dest_len when no same-family op exists");
}

#[test]
fn family_block_moves_are_doable_and_respect_eligibility() {
    let director = create_director(vec![
        Vehicle {
            visits: vec![900, 901, 250],
        },
        Vehicle {
            visits: vec![50, 150],
        },
    ]);

    // 900/901 are only eligible for owner 0, so no move should ever relocate
    // that block to vehicle 1.
    let selector = family_block_selector(2, eligible_owner_zero_only_for_900s);
    let moves: Vec<_> = selector.iter_moves(&director).collect();

    assert!(
        moves
            .iter()
            .all(|m| !(m.source_entity_index() == 0 && m.dest_entity_index() == 1)),
        "ineligible block must never target a restricted destination"
    );
    for m in &moves {
        assert!(m.is_doable(&director), "generated move should be doable: {m:?}");
    }
    assert_eq!(selector.size(&director), moves.len());
}

#[test]
fn family_block_no_op_relocation_is_skipped() {
    // A single vehicle with only one same-family block and nowhere else to land
    // (no other same-family op, no other entity) must not emit the intra-list
    // identity move (dest_position == source_start on the same entity).
    let director = create_director(vec![Vehicle {
        visits: vec![100, 101],
    }]);

    let selector = family_block_selector(2, eligible_any);
    let moves: Vec<_> = selector.iter_moves(&director).collect();

    assert!(
        moves.is_empty(),
        "the only reachable relocation is the intra-list identity no-op, so nothing should be emitted: {moves:?}"
    );
}
