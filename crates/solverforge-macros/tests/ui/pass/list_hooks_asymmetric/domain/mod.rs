solverforge::planning_model! {
    root = "crates/solverforge-macros/tests/ui/pass/list_hooks_asymmetric/domain";

    mod task;
    mod family_route;
    mod eligible_route;
    mod plan;

    pub use task::Task;
    pub use family_route::FamilyRoute;
    pub use eligible_route::EligibleRoute;
    pub use plan::Plan;
}
