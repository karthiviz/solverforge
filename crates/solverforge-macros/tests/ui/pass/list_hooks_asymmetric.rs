// Lagrange S8d review follow-up: proves the macro's `quote!{ None }` fallback in
// `planning_model/support.rs` (family_key_expr / eligible_owners_expr) genuinely
// compiles when only ONE of `element_family_key_fn` / `element_eligible_owners_fn`
// is set, for both directions. The existing `list_hooks.rs` fixture only ever sets
// both hooks together, so it never exercises either fallback branch.
#[path = "list_hooks_asymmetric/domain/mod.rs"]
mod domain;

use domain::*;
use solverforge::stream::CollectionExtract;

fn main() {
    let plan = Plan {
        tasks: Vec::new(),
        family_routes: Vec::new(),
        eligible_routes: Vec::new(),
        score: None,
    };

    let _ = Plan::tasks().extract(&plan);
    let _ = Plan::family_routes().extract(&plan);
    let _ = Plan::eligible_routes().extract(&plan);
}
