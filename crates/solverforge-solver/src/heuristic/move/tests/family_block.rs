// Tests for FamilyBlockMove operations.

use super::*;

#[derive(Clone, Debug)]
struct Vehicle {
    visits: Vec<i32>,
}

#[derive(Clone, Debug)]
struct RoutingSolution {
    vehicles: Vec<Vehicle>,
    score: Option<SoftScore>,
}

impl PlanningSolution for RoutingSolution {
    type Score = SoftScore;
    fn score(&self) -> Option<Self::Score> {
        self.score
    }
    fn set_score(&mut self, score: Option<Self::Score>) {
        self.score = score;
    }
}

fn get_vehicles(s: &RoutingSolution) -> &Vec<Vehicle> {
    &s.vehicles
}
fn get_vehicles_mut(s: &mut RoutingSolution) -> &mut Vec<Vehicle> {
    &mut s.vehicles
}

fn list_len(s: &RoutingSolution, entity_idx: usize) -> usize {
    s.vehicles.get(entity_idx).map_or(0, |v| v.visits.len())
}
fn list_get(s: &RoutingSolution, entity_idx: usize, pos: usize) -> Option<i32> {
    s.vehicles
        .get(entity_idx)
        .and_then(|v| v.visits.get(pos))
        .copied()
}
fn sublist_remove(
    s: &mut RoutingSolution,
    entity_idx: usize,
    start: usize,
    end: usize,
) -> Vec<i32> {
    s.vehicles
        .get_mut(entity_idx)
        .map(|v| v.visits.drain(start..end).collect())
        .unwrap_or_default()
}
fn sublist_insert(s: &mut RoutingSolution, entity_idx: usize, pos: usize, items: Vec<i32>) {
    if let Some(v) = s.vehicles.get_mut(entity_idx) {
        for (i, item) in items.into_iter().enumerate() {
            v.visits.insert(pos + i, item);
        }
    }
}

// Every op is eligible for any owner (empty eligible set).
fn eligible_any(_s: &RoutingSolution, _v: i32) -> Vec<usize> {
    Vec::new()
}

// Op value 2 is only eligible for owner 0; all other values are unrestricted.
fn eligible_only_entity_zero_for_value_2(_s: &RoutingSolution, v: i32) -> Vec<usize> {
    if v == 2 { vec![0] } else { Vec::new() }
}

fn create_director(vehicles: Vec<Vehicle>) -> ScoreDirector<RoutingSolution, ()> {
    let solution = RoutingSolution {
        vehicles,
        score: None,
    };
    let extractor = Box::new(EntityCollectionExtractor::new(
        "Vehicle",
        "vehicles",
        get_vehicles,
        get_vehicles_mut,
    ));
    let entity_desc = EntityDescriptor::new("Vehicle", TypeId::of::<Vehicle>(), "vehicles")
        .with_extractor(extractor);
    let descriptor = SolutionDescriptor::new("RoutingSolution", TypeId::of::<RoutingSolution>())
        .with_entity(entity_desc);
    ScoreDirector::simple(solution, descriptor, |s, _| s.vehicles.len())
}

#[test]
fn family_block_move_and_undo_round_trip() {
    let vehicles = vec![
        Vehicle {
            visits: vec![1, 2, 3, 4],
        },
        Vehicle {
            visits: vec![10, 20],
        },
    ];
    let mut director = create_director(vehicles);

    // Relocate block [0..2) from entity 0 to entity 1 at position 0.
    let m = FamilyBlockMove::<RoutingSolution, i32>::new(
        0,
        0,
        2,
        1,
        0,
        list_len,
        list_get,
        sublist_remove,
        sublist_insert,
        eligible_any,
        "visits",
        0,
    );

    assert!(m.is_doable(&director));

    {
        let mut recording = SnapshotDirector::new(&mut director);
        m.do_move(&mut recording);

        let sol = director.working_solution();
        assert_eq!(sol.vehicles[0].visits, vec![3, 4]);
        assert_eq!(sol.vehicles[1].visits, vec![1, 2, 10, 20]);

        recording.undo_changes();
    }

    let sol = director.working_solution();
    assert_eq!(sol.vehicles[0].visits, vec![1, 2, 3, 4]);
    assert_eq!(sol.vehicles[1].visits, vec![10, 20]);
}

#[test]
fn family_block_not_doable_when_dest_ineligible() {
    let vehicles = vec![
        Vehicle {
            visits: vec![1, 2, 3, 4],
        },
        Vehicle {
            visits: vec![10, 20],
        },
    ];
    let director = create_director(vehicles);

    // Block [0..2) contains value 2, which is only eligible for entity 0.
    let restricted = FamilyBlockMove::<RoutingSolution, i32>::new(
        0,
        0,
        2,
        1,
        0,
        list_len,
        list_get,
        sublist_remove,
        sublist_insert,
        eligible_only_entity_zero_for_value_2,
        "visits",
        0,
    );
    assert!(!restricted.is_doable(&director));

    // With an unrestricted eligibility fn, the same relocation is doable.
    let unrestricted = FamilyBlockMove::<RoutingSolution, i32>::new(
        0,
        0,
        2,
        1,
        0,
        list_len,
        list_get,
        sublist_remove,
        sublist_insert,
        eligible_any,
        "visits",
        0,
    );
    assert!(unrestricted.is_doable(&director));
}
