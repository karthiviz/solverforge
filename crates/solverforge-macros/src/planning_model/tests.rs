#[test]
fn expansion_tracks_every_manifest_module_as_include_dependency() {
    let expanded = expand(quote! {
        root = "tests/ui/pass/scalar_multi_module/domain";

        mod plan;
        mod task;
        mod worker;

        pub use plan::Plan;
        pub use task::Task;
        pub use worker::Worker;
    })
    .expect("planning_model! should expand")
    .to_string();

    assert!(expanded.contains("include_str !"));
    assert!(expanded.contains("plan.rs"));
    assert!(expanded.contains("task.rs"));
    assert!(expanded.contains("worker.rs"));
}

#[test]
fn expansion_attaches_list_order_and_precedence_hooks_to_existing_slots() {
    let expanded = expand(quote! {
        root = "tests/ui/pass/list_hooks/domain";

        mod operation;
        mod route;
        mod plan;

        pub use operation::Operation;
        pub use route::Route;
        pub use plan::Plan;
    })
    .expect("planning_model! should expand")
    .to_string();

    assert!(expanded.contains("__solverforge_runtime_list_construction_element_order_routes"));
    assert!(expanded.contains("__solverforge_runtime_list_precedence_duration_routes"));
    assert!(expanded.contains("__solverforge_runtime_list_precedence_successors_routes"));
    assert!(expanded.contains("slot = slot . with_construction_element_order_key"));
    assert!(expanded.contains("slot = slot . with_precedence_hooks"));
    assert!(expanded.contains("route :: operation_construction_order"));
    assert!(expanded.contains("route :: operation_duration"));
    assert!(expanded.contains("route :: operation_successors"));

    // Lagrange S8d: element_family_key_fn / element_eligible_owners_fn reach the
    // ListVariableSlot via the same attach_runtime_list_hooks mechanism as the
    // precedence hooks above.
    assert!(expanded.contains("__solverforge_runtime_list_element_family_key_routes"));
    assert!(expanded.contains("__solverforge_runtime_list_element_eligible_owners_routes"));
    assert!(expanded.contains("slot = slot . with_family_block_hooks"));
    assert!(expanded.contains("route :: operation_family_key"));
    assert!(expanded.contains("route :: operation_eligible_owners"));
}
