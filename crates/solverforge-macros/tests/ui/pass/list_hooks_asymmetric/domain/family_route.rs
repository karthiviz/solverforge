use solverforge::prelude::*;

/// Sets ONLY `element_family_key_fn` (no `element_eligible_owners_fn`) so the macro's
/// asymmetric-absence fallback for the eligible-owners half must produce `None`.
#[planning_entity]
pub struct FamilyRoute {
    #[planning_id]
    pub id: usize,

    #[planning_list_variable(element_collection = "tasks", element_family_key_fn = "task_family_key")]
    pub tasks: Vec<usize>,
}

pub(super) fn task_family_key(_plan: &super::Plan, task_id: usize) -> Option<u64> {
    Some(task_id as u64)
}
