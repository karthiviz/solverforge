use solverforge::prelude::*;

use super::{EligibleRoute, FamilyRoute, Task};

#[planning_solution]
pub struct Plan {
    #[problem_fact_collection]
    pub tasks: Vec<Task>,

    #[planning_entity_collection]
    pub family_routes: Vec<FamilyRoute>,

    #[planning_entity_collection]
    pub eligible_routes: Vec<EligibleRoute>,

    #[planning_score]
    pub score: Option<HardSoftScore>,
}
