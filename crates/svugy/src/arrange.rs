//! Search a model's arrangements for its most readable layout: layout lays each candidate out, geometry projects and measures it, and one score, straying from the author's order last, ranks them.

use std::cmp::Ordering;

use svugy_model::model::Model;
use svugy_model::path::ElementPath;

use crate::geometry::{QuantizedLength, Scene, SceneScore};
use crate::layout::{Arrangements, GroupId, LevelPlans, SlotSide};
use crate::metrics::TextMeasure;

/// Ranks one candidate arrangement, smaller reading better, compared field by field in declaration order: a hard violation outweighs any number of crossings, a crossing any amount of hidden ink, and so on down to total length.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct LayoutScore {
    violations: usize,
    crossings: usize,
    hidden_junctions: usize,
    arc_length_over_solids: QuantizedLength,
    hidden_trace_length: QuantizedLength,
    arcs: usize,
    bends: usize,
    source_order_distance: usize,
    total_length: QuantizedLength,
}

/// Searches a model's arrangements for its most readable layout: starting from the seed layout's, it tries every move of one decision at a time, keeps a move only when it scores strictly better, and stops when a sweep improves nothing or its layouts run out.
pub(crate) struct ArrangementSearch<'s, M> {
    model: &'s Model,
    measure: &'s M,
    layouts_left: usize,
}

/// Holds one laid out arrangement with its score.
struct Candidate {
    arrangements: Arrangements,
    plans: LevelPlans,
    score: LayoutScore,
}

impl LayoutScore {
    /// Rank a candidate by what its projected scene costs, `scene_score`, and by `source_order_distance`, the sibling pairs it packs against the author's order.
    pub(crate) fn new(scene_score: SceneScore, source_order_distance: usize) -> Self {
        let SceneScore {
            violations,
            crossings,
            hidden_junctions,
            arc_length_over_solids,
            hidden_trace_length,
            arcs,
            bends,
            total_length,
        } = scene_score;
        Self {
            violations,
            crossings,
            hidden_junctions,
            arc_length_over_solids,
            hidden_trace_length,
            arcs,
            bends,
            source_order_distance,
            total_length,
        }
    }
}

impl<'s, M: TextMeasure> ArrangementSearch<'s, M> {
    /// Layouts one search may run, the seed's included, so a busy diagram still renders at interactive speed.
    const LAYOUT_BUDGET: usize = 240;
    /// Most children a container may hold for every order of them to be tried; larger ones try adjacent swaps.
    const MAX_PERMUTED_CHILDREN: usize = 6;

    /// Search `model`'s arrangements, measuring labels with `measure`.
    pub(crate) fn new(model: &'s Model, measure: &'s M) -> Self {
        let layouts_left = Self::LAYOUT_BUDGET;
        Self {
            model,
            measure,
            layouts_left,
        }
    }

    /// Return the most readable arrangements found, with their layout: sweep every free container's orders and orientations, then every bundle's acceptance and slot side, until a sweep improves nothing.
    pub(crate) fn best(mut self) -> (Arrangements, LevelPlans) {
        let (seed_plans, seed_arrangements) = LevelPlans::lay_out_seeded(self.model, self.measure);
        self.layouts_left -= 1;
        let seed_score = self.score_of(&seed_arrangements, &seed_plans);
        let mut best = Candidate {
            arrangements: seed_arrangements,
            plans: seed_plans,
            score: seed_score,
        };
        let free_containers = Arrangements::free_containers(self.model);
        let bundle_groups = GroupId::bundle_candidates_of(self.model);
        loop {
            let mut is_improved = false;
            for container in &free_containers {
                let container_moves = self.moves_of_container(&best.arrangements, container);
                is_improved |= self.keep_best_of(container_moves, &mut best);
            }
            for group in &bundle_groups {
                let group_moves = Self::moves_of_group(&best.arrangements, group);
                is_improved |= self.keep_best_of(group_moves, &mut best);
            }
            let is_exhausted = self.layouts_left == 0;
            if !is_improved || is_exhausted {
                break;
            }
        }
        (best.arrangements, best.plans)
    }

    /// Lay out every one of `moves` the budget allows and keep in `best` each one scoring strictly better than it, ties keeping the earlier; tell whether any did.
    fn keep_best_of(&mut self, moves: Vec<Arrangements>, best: &mut Candidate) -> bool {
        let mut is_improved = false;
        for arrangements in moves {
            let is_exhausted = self.layouts_left == 0;
            if is_exhausted {
                break;
            }
            let is_current = arrangements == best.arrangements;
            if is_current {
                continue;
            }
            self.layouts_left -= 1;
            let plans = LevelPlans::lay_out_with(self.model, &arrangements, self.measure);
            let score = self.score_of(&arrangements, &plans);
            let is_better = score.cmp(&best.score) == Ordering::Less;
            if !is_better {
                continue;
            }
            *best = Candidate {
                arrangements,
                plans,
                score,
            };
            is_improved = true;
        }
        is_improved
    }

    /// Return `current` with `container` in each order and each orientation to try: every order of up to `MAX_PERMUTED_CHILDREN` children, else its current order with each adjacent pair swapped.
    fn moves_of_container(
        &self,
        current: &Arrangements,
        container: &ElementPath,
    ) -> Vec<Arrangements> {
        let current_order = current.packing_order_of(self.model, container);
        let child_count = current_order.len();
        let is_permutable = child_count <= Self::MAX_PERMUTED_CHILDREN;
        let orders = if is_permutable {
            Self::permutations_of(&current_order)
        } else {
            Self::adjacent_swaps_of(&current_order)
        };
        let mut moves = Vec::new();
        for order in orders {
            let ordered = current
                .clone()
                .with_packing_order(container.clone(), order);
            let in_row = ordered.clone().with_row(container);
            let in_column = ordered.with_column(container.clone());
            moves.push(in_row);
            moves.push(in_column);
        }
        moves
    }

    /// Return `current` with `group` unbundled, and with its slots on each side.
    fn moves_of_group(current: &Arrangements, group: &GroupId) -> Vec<Arrangements> {
        let unbundled = current.clone().with_unbundled_group(group.clone());
        let slot_sides = [SlotSide::TowardFarChild, SlotSide::TowardPlanEastOrSouth];
        let slotted = slot_sides
            .into_iter()
            .map(|slot_side| current.clone().with_slot_side(group.clone(), slot_side));
        let mut moves = vec![unbundled];
        moves.extend(slotted);
        moves
    }

    /// Score `arrangements` laid out as `plans`: project the scene, measure it, and add how far the arrangement strays from source order.
    fn score_of(&self, arrangements: &Arrangements, plans: &LevelPlans) -> LayoutScore {
        let scene = Scene::project(self.model, plans, self.measure);
        let scene_score = scene.score();
        let source_order_distance = arrangements.source_order_distance(self.model);
        LayoutScore::new(scene_score, source_order_distance)
    }

    /// Return every order of `children`, in lexicographic order of their positions, `children` itself first.
    fn permutations_of(children: &[ElementPath]) -> Vec<Vec<ElementPath>> {
        let is_empty = children.is_empty();
        if is_empty {
            return vec![Vec::new()];
        }
        let mut permutations = Vec::new();
        for leading_index in 0..children.len() {
            let mut remaining_children = children.to_vec();
            let leading_child = remaining_children.remove(leading_index);
            let tail_permutations = Self::permutations_of(&remaining_children);
            for tail in tail_permutations {
                let mut permutation = vec![leading_child.clone()];
                permutation.extend(tail);
                permutations.push(permutation);
            }
        }
        permutations
    }

    /// Return `order` with each adjacent pair swapped in turn, `order` itself first.
    fn adjacent_swaps_of(order: &[ElementPath]) -> Vec<Vec<ElementPath>> {
        let unswapped = order.to_vec();
        let swapped = (1..order.len()).map(|swap_index| {
            let mut swapped_order = order.to_vec();
            swapped_order.swap(swap_index - 1, swap_index);
            swapped_order
        });
        std::iter::once(unswapped).chain(swapped).collect()
    }
}

#[cfg(test)]
mod tests {
    use svugy_parse::diagnostic::SourceLanguage;
    use svugy_parse::markdown::MarkdownLanguage;

    use super::*;
    use crate::metrics::BundledFont;

    #[test]
    fn the_search_never_returns_a_layout_scoring_worse_than_its_seed() {
        let examples = [
            ("cluster", include_str!("../../../examples/cluster/cluster.md")),
            ("events", include_str!("../../../examples/events/events.md")),
            ("release", include_str!("../../../examples/release/release.md")),
            ("svugy", include_str!("../../../examples/svugy/svugy.md")),
            ("webapp", include_str!("../../../examples/webapp/webapp.md")),
        ];
        let font = BundledFont::new();
        for (name, markdown) in examples {
            let model = example_model(markdown);
            let search = ArrangementSearch::new(&model, &font);

            let (seed_plans, seed_arrangements) = LevelPlans::lay_out_seeded(&model, &font);
            let seed_score = search.score_of(&seed_arrangements, &seed_plans);
            let (best_arrangements, best_plans) = search.best();
            let rescoring_search = ArrangementSearch::new(&model, &font);
            let best_score = rescoring_search.score_of(&best_arrangements, &best_plans);

            assert!(
                best_score <= seed_score,
                "{name}: the search keeps a move only when it scores better, so it never ends worse than its seed: {best_score:?} > {seed_score:?}"
            );
        }
    }

    fn example_model(markdown: &str) -> Model {
        let import = MarkdownLanguage::import(markdown);
        let import_parts = import.into_admitted_parts();
        import_parts.model.expect("every example is a valid diagram")
    }
}
