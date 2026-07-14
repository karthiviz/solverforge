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

// Lagrange S8d review follow-up: the test above only exercises the "both hooks set"
// branch of the `family_key_expr` / `eligible_owners_expr` codegen in
// `support.rs`. Each of those two `if let` chains falls back to a bare
// `quote! { None }` when its own hook attribute is absent -- but that fallback was
// never previously exercised through actual macro expansion (only through the
// `ListVariableSlot::with_family_block_hooks` builder called directly). This test
// expands a domain where the two hooks are set on two DIFFERENT entities, each
// missing the other hook, and pins the exact generated `with_family_block_hooks`
// call (including argument order/position) for both asymmetric directions.
#[test]
fn expansion_falls_back_to_none_for_the_absent_family_block_hook_in_each_direction() {
    let expanded = expand(quote! {
        root = "tests/ui/pass/list_hooks_asymmetric/domain";

        mod task;
        mod family_route;
        mod eligible_route;
        mod plan;

        pub use task::Task;
        pub use family_route::FamilyRoute;
        pub use eligible_route::EligibleRoute;
        pub use plan::Plan;
    })
    .expect("planning_model! should expand")
    .to_string();

    // Family-only entity (`FamilyRoute`, solution field `family_routes`): the
    // family-key helper is generated and wired as `Some`, while the
    // eligible-owners argument must fall back to `None` -- no eligible-owners
    // helper is generated for this entity at all.
    assert!(expanded.contains("__solverforge_runtime_list_element_family_key_family_routes"));
    assert!(!expanded.contains("__solverforge_runtime_list_element_eligible_owners_family_routes"));
    assert!(expanded.contains(
        "slot = slot . with_family_block_hooks (:: core :: option :: Option :: Some \
         (__solverforge_runtime_list_element_family_key_family_routes) , \
         :: core :: option :: Option :: None)"
    ));

    // Eligible-only entity (`EligibleRoute`, solution field `eligible_routes`): the
    // mirror image -- eligible-owners helper wired as `Some`, family-key argument
    // falls back to `None`, no family-key helper generated for this entity.
    assert!(expanded.contains("__solverforge_runtime_list_element_eligible_owners_eligible_routes"));
    assert!(!expanded.contains("__solverforge_runtime_list_element_family_key_eligible_routes"));
    assert!(expanded.contains(
        "slot = slot . with_family_block_hooks (:: core :: option :: Option :: None , \
         :: core :: option :: Option :: Some \
         (__solverforge_runtime_list_element_eligible_owners_eligible_routes))"
    ));
}
