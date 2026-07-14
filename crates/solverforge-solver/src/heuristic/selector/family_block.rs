/* Family-block move selector for atomic same-family segment relocation.

Generates `FamilyBlockMove`s that relocate maximal contiguous runs of same-family
list elements (per `element_family_key_fn`) as a single atomic block, gated by
per-element owner eligibility (`element_eligible_owners_fn`). Unlike
`SublistChangeMoveSelector`, segment boundaries are not swept across all sizes —
only maximal same-family runs of length >= `min_block_size` are ever emitted
(single-op runs are never a candidate block), and destination insertion position
is chosen deterministically (family-adjacent, else append) rather than swept
across every position.

# Example

```
use solverforge_solver::heuristic::selector::family_block::FamilyBlockMoveSelector;
use solverforge_solver::heuristic::selector::entity::FromSolutionEntitySelector;
use solverforge_solver::heuristic::selector::MoveSelector;
use solverforge_core::domain::PlanningSolution;
use solverforge_core::score::SoftScore;

#[derive(Clone, Debug)]
struct Vehicle { visits: Vec<i32> }

#[derive(Clone, Debug)]
struct Solution { vehicles: Vec<Vehicle>, score: Option<SoftScore> }

impl PlanningSolution for Solution {
type Score = SoftScore;
fn score(&self) -> Option<Self::Score> { self.score }
fn set_score(&mut self, score: Option<Self::Score>) { self.score = score; }
}

fn list_len(s: &Solution, entity_idx: usize) -> usize {
s.vehicles.get(entity_idx).map_or(0, |v| v.visits.len())
}
fn list_get(s: &Solution, entity_idx: usize, pos: usize) -> Option<i32> {
s.vehicles.get(entity_idx).and_then(|v| v.visits.get(pos)).copied()
}
fn sublist_remove(s: &mut Solution, entity_idx: usize, start: usize, end: usize) -> Vec<i32> {
s.vehicles.get_mut(entity_idx)
.map(|v| v.visits.drain(start..end).collect())
.unwrap_or_default()
}
fn sublist_insert(s: &mut Solution, entity_idx: usize, pos: usize, items: Vec<i32>) {
if let Some(v) = s.vehicles.get_mut(entity_idx) {
for (i, item) in items.into_iter().enumerate() {
v.visits.insert(pos + i, item);
}
}
}
fn family_key(_s: &Solution, v: i32) -> Option<u64> { Some(v as u64 % 10) }
fn eligible_owners(_s: &Solution, _v: i32) -> Vec<usize> { Vec::new() }

let selector = FamilyBlockMoveSelector::<Solution, i32, _>::new(
FromSolutionEntitySelector::new(0),
2,
list_len,
list_get,
sublist_remove,
sublist_insert,
family_key,
eligible_owners,
"visits",
0,
);
```
*/

use std::fmt::Debug;
use std::marker::PhantomData;

use solverforge_core::domain::PlanningSolution;
use solverforge_scoring::Director;

use crate::heuristic::r#move::FamilyBlockMove;

use super::entity::EntitySelector;
use super::list_support::collect_selected_entities;
use super::move_selector::{
    CandidateId, CandidateStore, MoveCandidateRef, MoveCursor, MoveSelector, MoveStreamContext,
};

/// A move selector that generates family-block moves — relocating a maximal
/// same-family contiguous run atomically, gated by per-element owner eligibility.
///
/// # Type Parameters
/// * `S` - The solution type
/// * `V` - The list element type
/// * `ES` - The entity selector type
pub struct FamilyBlockMoveSelector<S, V, ES> {
    entity_selector: ES,
    // Minimum contiguous same-family block size (inclusive). Usually 2.
    min_block_size: usize,
    list_len: fn(&S, usize) -> usize,
    list_get: fn(&S, usize, usize) -> Option<V>,
    sublist_remove: fn(&mut S, usize, usize, usize) -> Vec<V>,
    sublist_insert: fn(&mut S, usize, usize, Vec<V>),
    element_family_key_fn: fn(&S, V) -> Option<u64>,
    element_eligible_owners_fn: fn(&S, V) -> Vec<usize>,
    variable_name: &'static str,
    descriptor_index: usize,
    _phantom: PhantomData<(fn() -> S, fn() -> V)>,
}

impl<S, V, ES> FamilyBlockMoveSelector<S, V, ES> {
    /* Creates a new family-block move selector.

    # Arguments
    * `entity_selector` - Selects entities to generate moves for
    * `min_block_size` - Minimum contiguous same-family block length (must be >= 1)
    * `list_len` - Function to get list length
    * `list_get` - Function to read the element at a position
    * `sublist_remove` - Function to drain a range `[start, end)`, returning removed elements
    * `sublist_insert` - Function to insert a slice at a position
    * `element_family_key_fn` - Family key for an element; `None` never joins a block
    * `element_eligible_owners_fn` - Eligible owner entity indices for an element
      (empty vec = any owner is eligible)
    * `variable_name` - Name of the list variable
    * `descriptor_index` - Entity descriptor index

    # Panics
    Panics if `min_block_size == 0`.
    */
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        entity_selector: ES,
        min_block_size: usize,
        list_len: fn(&S, usize) -> usize,
        list_get: fn(&S, usize, usize) -> Option<V>,
        sublist_remove: fn(&mut S, usize, usize, usize) -> Vec<V>,
        sublist_insert: fn(&mut S, usize, usize, Vec<V>),
        element_family_key_fn: fn(&S, V) -> Option<u64>,
        element_eligible_owners_fn: fn(&S, V) -> Vec<usize>,
        variable_name: &'static str,
        descriptor_index: usize,
    ) -> Self {
        assert!(min_block_size >= 1, "min_block_size must be at least 1");
        Self {
            entity_selector,
            min_block_size,
            list_len,
            list_get,
            sublist_remove,
            sublist_insert,
            element_family_key_fn,
            element_eligible_owners_fn,
            variable_name,
            descriptor_index,
            _phantom: PhantomData,
        }
    }

    /// Maximal same-family runs of length >= min_block_size in one entity's list
    /// (start, end-exclusive). A `None` family key never joins a run.
    fn blocks_in(&self, solution: &S, entity: usize) -> Vec<(usize, usize)> {
        let len = (self.list_len)(solution, entity);
        let mut out = Vec::new();
        let mut i = 0;
        while i < len {
            let fam = (self.list_get)(solution, entity, i)
                .and_then(|v| (self.element_family_key_fn)(solution, v));
            if fam.is_none() {
                i += 1;
                continue;
            }
            let mut j = i + 1;
            while j < len {
                let f2 = (self.list_get)(solution, entity, j)
                    .and_then(|v| (self.element_family_key_fn)(solution, v));
                if f2 != fam {
                    break;
                }
                j += 1;
            }
            if j - i >= self.min_block_size {
                out.push((i, j));
            }
            i = j;
        }
        out
    }

    /// Every op in `[start, end)` on `entity` must be eligible for `dst_entity`
    /// (empty eligible set => any owner allowed).
    fn block_eligible_for(
        &self,
        solution: &S,
        entity: usize,
        start: usize,
        end: usize,
        dst_entity: usize,
    ) -> bool {
        for pos in start..end {
            if let Some(v) = (self.list_get)(solution, entity, pos) {
                let eligible = (self.element_eligible_owners_fn)(solution, v);
                if !eligible.is_empty() && !eligible.contains(&dst_entity) {
                    return false;
                }
            }
        }
        true
    }

    /// Lowest position adjacent to (immediately after) a same-family op on `dst`,
    /// else append at the end (deterministic, seed-independent).
    ///
    /// `exclude` is the source block's own range when `dst` is the source entity
    /// (intra-list relocation) — positions in that range are skipped and never
    /// consume a post-removal index, since `FamilyBlockMove::dest_position` is
    /// relative to the list *after* the block has been removed.
    fn insert_pos(
        &self,
        solution: &S,
        dst: usize,
        bfam: u64,
        exclude: Option<(usize, usize)>,
    ) -> usize {
        let dlen = (self.list_len)(solution, dst);
        let mut post_removal_idx = 0usize;
        for p in 0..dlen {
            if let Some((ex_start, ex_end)) = exclude {
                if p >= ex_start && p < ex_end {
                    continue;
                }
            }
            let f = (self.list_get)(solution, dst, p)
                .and_then(|v| (self.element_family_key_fn)(solution, v));
            if f == Some(bfam) {
                return post_removal_idx + 1;
            }
            post_removal_idx += 1;
        }
        post_removal_idx // fallback: append (seed-independent)
    }
}

impl<S, V: Debug, ES: Debug> Debug for FamilyBlockMoveSelector<S, V, ES> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FamilyBlockMoveSelector")
            .field("entity_selector", &self.entity_selector)
            .field("min_block_size", &self.min_block_size)
            .field("variable_name", &self.variable_name)
            .field("descriptor_index", &self.descriptor_index)
            .finish()
    }
}

pub struct FamilyBlockMoveCursor<S, V>
where
    S: PlanningSolution,
    V: Clone + PartialEq + Send + Sync + Debug + 'static,
{
    store: CandidateStore<S, FamilyBlockMove<S, V>>,
    next_index: usize,
}

impl<S, V> FamilyBlockMoveCursor<S, V>
where
    S: PlanningSolution,
    V: Clone + PartialEq + Send + Sync + Debug + 'static,
{
    fn new(store: CandidateStore<S, FamilyBlockMove<S, V>>) -> Self {
        Self {
            store,
            next_index: 0,
        }
    }
}

impl<S, V> MoveCursor<S, FamilyBlockMove<S, V>> for FamilyBlockMoveCursor<S, V>
where
    S: PlanningSolution,
    V: Clone + PartialEq + Send + Sync + Debug + 'static,
{
    fn next_candidate(&mut self) -> Option<CandidateId> {
        while self.next_index < self.store.len() {
            let id = CandidateId::new(self.next_index);
            self.next_index += 1;
            if self.store.candidate(id).is_some() {
                return Some(id);
            }
        }
        None
    }

    fn candidate(&self, id: CandidateId) -> Option<MoveCandidateRef<'_, S, FamilyBlockMove<S, V>>> {
        self.store.candidate(id)
    }

    fn take_candidate(&mut self, id: CandidateId) -> FamilyBlockMove<S, V> {
        self.store.take_candidate(id)
    }
}

impl<S, V, ES> MoveSelector<S, FamilyBlockMove<S, V>> for FamilyBlockMoveSelector<S, V, ES>
where
    S: PlanningSolution,
    V: Clone + PartialEq + Send + Sync + Debug + 'static,
    ES: EntitySelector<S>,
{
    type Cursor<'a>
        = FamilyBlockMoveCursor<S, V>
    where
        Self: 'a;

    fn open_cursor<'a, D: Director<S>>(&'a self, score_director: &D) -> Self::Cursor<'a> {
        self.open_cursor_with_context(score_director, MoveStreamContext::default())
    }

    fn open_cursor_with_context<'a, D: Director<S>>(
        &'a self,
        score_director: &D,
        _context: MoveStreamContext,
    ) -> Self::Cursor<'a> {
        let selected =
            collect_selected_entities(&self.entity_selector, score_director, self.list_len);
        let solution = score_director.working_solution();
        let mut store = CandidateStore::new();
        for &src_entity in &selected.entities {
            for (block_start, block_end) in self.blocks_in(solution, src_entity) {
                let Some(bfam) = (self.list_get)(solution, src_entity, block_start)
                    .and_then(|v| (self.element_family_key_fn)(solution, v))
                else {
                    continue;
                };
                for &dst_entity in &selected.entities {
                    if !self.block_eligible_for(
                        solution,
                        src_entity,
                        block_start,
                        block_end,
                        dst_entity,
                    ) {
                        continue;
                    }
                    let exclude = (dst_entity == src_entity).then_some((block_start, block_end));
                    let dst_pos = self.insert_pos(solution, dst_entity, bfam, exclude);
                    if dst_entity == src_entity && dst_pos == block_start {
                        continue; // no-op
                    }
                    store.push(FamilyBlockMove::new(
                        src_entity,
                        block_start,
                        block_end,
                        dst_entity,
                        dst_pos,
                        self.list_len,
                        self.list_get,
                        self.sublist_remove,
                        self.sublist_insert,
                        self.element_eligible_owners_fn,
                        self.variable_name,
                        self.descriptor_index,
                    ));
                }
            }
        }
        FamilyBlockMoveCursor::new(store)
    }

    fn size<D: Director<S>>(&self, score_director: &D) -> usize {
        let mut cursor = self.open_cursor(score_director);
        let mut count = 0;
        while cursor.next_candidate().is_some() {
            count += 1;
        }
        count
    }
}
