//! Search a model's arrangements for its most readable layout: layout lays each candidate out, geometry projects and measures it, and one score, straying from the author's order last, ranks them.

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::marker::PhantomData;

use svugy_model::model::Model;
use svugy_model::path::ElementPath;

use crate::geometry::{Camera, QuantizedLength, Scene, SceneScore};
use crate::layout::{Arrangements, GroupId, LevelPlans, PlacementCache, RankMove, SlotSide};
use crate::metrics::TextMeasure;

/// Ranks one candidate arrangement, smaller reading better, in four tiers compared in order: hard violations, then crossings, then ink, the drawn length with every other visual cost priced in length, then the sibling pairs packed against the author's order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct LayoutScore {
    violations: usize,
    crossings: usize,
    ink: QuantizedLength,
    source_order_distance: usize,
}

/// Searches a model's arrangements for its most readable layout as camera `C` sees it: starting from the seed layout's, it tries every move of one decision at a time, keeps a move only when it scores strictly better, and stops when a sweep improves nothing or its layouts run out.
pub(crate) struct ArrangementSearch<'s, M, C> {
    model: &'s Model,
    measure: &'s M,
    layouts_left: usize,
    placement_cache: PlacementCache,
    evaluated: BTreeSet<Arrangements>,
    scored_layouts: Vec<(LevelPlans, SceneScore)>,
    camera: PhantomData<C>,
}

/// Holds one laid out arrangement with its score.
struct Candidate {
    arrangements: Arrangements,
    plans: LevelPlans,
    score: LayoutScore,
}

impl LayoutScore {
    /// Screen units of drawn length one unit of arc over a solid adds on top of its own length.
    const ARC_OVER_SOLID_PRICE: f64 = 0.5;
    /// Screen units of drawn length one unit of hidden trace adds on top of its own length.
    const HIDDEN_TRACE_PRICE: f64 = 2.0;
    /// Screen units of drawn length one bend costs.
    const BEND_PRICE: f64 = 15.0;
    /// Screen units of drawn length one arc, a trunk or a lone arc, costs as a separate stroke.
    const ARC_PRICE: f64 = 30.0;
    /// Screen units of drawn length one hidden junction costs.
    const HIDDEN_JUNCTION_PRICE: f64 = 40.0;

    /// Rank a candidate by what its projected scene costs, `scene_score`, and by `source_order_distance`, the sibling pairs it packs against the author's order.
    // ink = L + ½·A + 2·H + 15·b + 30·a + 40·j: L total length, A arc over solids, H hidden trace, b bends, a arcs, j hidden junctions
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
        let total_length_units = total_length.units();
        let arc_over_solids_units = arc_length_over_solids.units();
        let hidden_trace_units = hidden_trace_length.units();
        let arc_over_solids_cost = Self::ARC_OVER_SOLID_PRICE * arc_over_solids_units;
        let hidden_trace_cost = Self::HIDDEN_TRACE_PRICE * hidden_trace_units;
        let bend_cost = Self::BEND_PRICE * bends as f64;
        let arc_cost = Self::ARC_PRICE * arcs as f64;
        let hidden_junction_cost = Self::HIDDEN_JUNCTION_PRICE * hidden_junctions as f64;
        let ink_units = total_length_units
            + arc_over_solids_cost
            + hidden_trace_cost
            + bend_cost
            + arc_cost
            + hidden_junction_cost;
        let ink = QuantizedLength::of(ink_units);
        Self {
            violations,
            crossings,
            ink,
            source_order_distance,
        }
    }
}

impl<'s, M: TextMeasure, C: Camera> ArrangementSearch<'s, M, C> {
    /// Layouts one search may run, the seed's included, so a busy diagram still renders at interactive speed.
    const LAYOUT_BUDGET: usize = 600;
    /// Most children a container may hold for every order of them to be tried; larger ones try adjacent swaps.
    const MAX_PERMUTED_CHILDREN: usize = 6;

    /// Search `model`'s arrangements, measuring labels with `measure`.
    pub(crate) fn new(model: &'s Model, measure: &'s M) -> Self {
        let layouts_left = Self::LAYOUT_BUDGET;
        let placement_cache = PlacementCache::new();
        let evaluated = BTreeSet::new();
        let scored_layouts = Vec::new();
        Self {
            model,
            measure,
            layouts_left,
            placement_cache,
            evaluated,
            scored_layouts,
            camera: PhantomData,
        }
    }

    /// Return the most readable arrangements found, with their layout: sweep every ranked level's moves within ranks, every free container's orders and orientations, then every bundle's acceptance and slot side; when a sweep improves nothing, try each container move paired with each rank move, since a container's new order often pays only once its neighbours move too; stop when that improves nothing either.
    pub(crate) fn best(mut self) -> (Arrangements, LevelPlans) {
        let (seed_plans, seed_arrangements) = LevelPlans::lay_out_seeded(self.model, self.measure);
        self.layouts_left -= 1;
        let seed_scene_score = self.remembered_scene_score_of(&seed_plans);
        let seed_source_order_distance = seed_arrangements.source_order_distance(self.model);
        let seed_score = LayoutScore::new(seed_scene_score, seed_source_order_distance);
        let evaluated_seed = seed_arrangements.clone();
        self.evaluated.insert(evaluated_seed);
        let mut best = Candidate {
            arrangements: seed_arrangements,
            plans: seed_plans,
            score: seed_score,
        };
        let rank_movable_levels = Arrangements::rank_movable_levels(self.model);
        let free_containers = Arrangements::free_containers(self.model);
        let bundle_groups = GroupId::bundle_candidates_of(self.model);
        loop {
            let mut is_improved = false;
            for level in &rank_movable_levels {
                let level_moves = self.moves_within_ranks_of(&best.arrangements, level);
                is_improved |= self.keep_best_of(level_moves, &mut best);
            }
            for container in &free_containers {
                let container_moves = self.moves_of_container(&best.arrangements, container);
                is_improved |= self.keep_best_of(container_moves, &mut best);
            }
            for group in &bundle_groups {
                let group_moves = Self::moves_of_group(&best.arrangements, group);
                is_improved |= self.keep_best_of(group_moves, &mut best);
            }
            let is_stuck = !is_improved;
            if is_stuck {
                let paired_moves = self.paired_moves_of(
                    &best.arrangements,
                    &free_containers,
                    &rank_movable_levels,
                );
                is_improved = self.keep_best_of(paired_moves, &mut best);
            }
            let is_exhausted = self.layouts_left == 0;
            if !is_improved || is_exhausted {
                break;
            }
        }
        (best.arrangements, best.plans)
    }

    /// Lay out every one of `moves` not evaluated before that the budget allows and keep in `best` each one scoring strictly better than it, ties keeping the earlier; tell whether any did. An arrangement evaluated before cannot win: it scored no better than a best that has only improved since. A layout equal to one already scored reuses its scene score instead of projecting it again.
    fn keep_best_of(&mut self, moves: Vec<Arrangements>, best: &mut Candidate) -> bool {
        let mut is_improved = false;
        for arrangements in moves {
            let is_exhausted = self.layouts_left == 0;
            if is_exhausted {
                break;
            }
            let is_evaluated = self.evaluated.contains(&arrangements);
            if is_evaluated {
                continue;
            }
            let evaluated_arrangements = arrangements.clone();
            self.evaluated.insert(evaluated_arrangements);
            self.layouts_left -= 1;
            let plans = LevelPlans::lay_out_with(
                self.model,
                &arrangements,
                &self.placement_cache,
                self.measure,
            );
            let scene_score = self.remembered_scene_score_of(&plans);
            let source_order_distance = arrangements.source_order_distance(self.model);
            let score = LayoutScore::new(scene_score, source_order_distance);
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
            let ordered = current.clone().with_packing_order(container.clone(), order);
            let in_row = ordered.clone().with_row(container);
            let in_column = ordered.with_column(container.clone());
            moves.push(in_row);
            moves.push(in_column);
        }
        moves
    }

    /// Return `current` with each move of a free container combined with each move within the ranks of every rank movable level.
    fn paired_moves_of(
        &self,
        current: &Arrangements,
        free_containers: &BTreeSet<ElementPath>,
        rank_movable_levels: &[ElementPath],
    ) -> Vec<Arrangements> {
        let mut paired_moves = Vec::new();
        for container in free_containers {
            let container_moves = self.moves_of_container(current, container);
            for container_move in container_moves {
                for level in rank_movable_levels {
                    let level_moves = self.moves_within_ranks_of(&container_move, level);
                    paired_moves.extend(level_moves);
                }
            }
        }
        paired_moves
    }

    /// Return `current` with each child of the ranked `level` moved to each end of its rank band and past its south neighbour.
    fn moves_within_ranks_of(
        &self,
        current: &Arrangements,
        level: &ElementPath,
    ) -> Vec<Arrangements> {
        let mut moves = Vec::new();
        for (child, _element) in self.model.children(level) {
            let rank_moves = [
                RankMove::ToNorthEnd(child.clone()),
                RankMove::ToSouthEnd(child.clone()),
                RankMove::SwapWithSouthNeighbour(child.clone()),
            ];
            for rank_move in rank_moves {
                let moved = current.clone().with_rank_move(level.clone(), rank_move);
                moves.push(moved);
            }
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

    /// Return the scene score of a layout equal to `plans` already scored, else score `plans` and remember it: many moves change nothing, so many candidates repeat a layout.
    fn remembered_scene_score_of(&mut self, plans: &LevelPlans) -> SceneScore {
        let scored_layout = self
            .scored_layouts
            .iter()
            .find(|(scored_plans, _scene_score)| scored_plans == plans);
        if let Some((_scored_plans, scene_score)) = scored_layout {
            return *scene_score;
        }
        let scene_score = self.scene_score_of(plans);
        let scored_plans = plans.clone();
        self.scored_layouts.push((scored_plans, scene_score));
        scene_score
    }

    /// Measure what the scene `plans` lay out costs its reader: project it and score it.
    fn scene_score_of(&self, plans: &LevelPlans) -> SceneScore {
        let scene = Scene::project::<C>(self.model, plans, self.measure);
        scene.score::<C>()
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
    use crate::geometry::Isometric;
    use crate::metrics::{BundledFont, InterFaces};

    #[test]
    fn the_search_never_returns_a_layout_scoring_worse_than_its_seed() {
        let examples = [
            (
                "cluster",
                include_str!("../../../examples/cluster/cluster.md"),
            ),
            ("events", include_str!("../../../examples/events/events.md")),
            (
                "release",
                include_str!("../../../examples/release/release.md"),
            ),
            ("svugy", include_str!("../../../examples/svugy/svugy.md")),
            ("webapp", include_str!("../../../examples/webapp/webapp.md")),
        ];
        let font = BundledFont::<InterFaces>::new();
        for (name, markdown) in examples {
            let model = example_model(markdown);
            let search = ArrangementSearch::<_, Isometric>::new(&model, &font);

            let (seed_plans, seed_arrangements) = LevelPlans::lay_out_seeded(&model, &font);
            let seed_score = layout_score_of(&search, &seed_arrangements, &seed_plans);
            let (best_arrangements, best_plans) = search.best();
            let rescoring_search = ArrangementSearch::<_, Isometric>::new(&model, &font);
            let best_score = layout_score_of(&rescoring_search, &best_arrangements, &best_plans);

            assert!(
                best_score <= seed_score,
                "{name}: the search keeps a move only when it scores better, so it never ends worse than its seed: {best_score:?} > {seed_score:?}"
            );
        }
    }

    fn layout_score_of(
        search: &ArrangementSearch<BundledFont<InterFaces>, Isometric>,
        arrangements: &Arrangements,
        plans: &LevelPlans,
    ) -> LayoutScore {
        let scene_score = search.scene_score_of(plans);
        let source_order_distance = arrangements.source_order_distance(search.model);
        LayoutScore::new(scene_score, source_order_distance)
    }

    fn example_model(markdown: &str) -> Model {
        let import = MarkdownLanguage::import(markdown);
        let import_parts = import.into_admitted_parts();
        import_parts
            .model
            .expect("every example is a valid diagram")
    }
}
