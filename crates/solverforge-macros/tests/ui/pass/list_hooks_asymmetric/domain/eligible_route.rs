use solverforge::prelude::*;

/// Sets ONLY `element_eligible_owners_fn` (no `element_family_key_fn`) so the macro's
/// asymmetric-absence fallback for the family-key half must produce `None`.
#[planning_entity]
pub struct EligibleRoute {
    #[planning_id]
    pub id: usize,

    #[planning_list_variable(element_collection = "tasks", element_eligible_owners_fn = "task_eligible_owners")]
    pub tasks: Vec<usize>,
}

pub(super) fn task_eligible_owners(_plan: &super::Plan, task_id: usize) -> Vec<usize> {
    vec![task_id]
}
