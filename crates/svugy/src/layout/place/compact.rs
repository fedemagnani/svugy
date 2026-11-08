//! Rearrange a level's resized children in the order the deepest layer placed them, so every layer shows the arrangement its reader already saw.

use std::collections::BTreeMap;

use kurbo::{Rect, Size, Vec2};
use svugy_model::path::ElementPath;

use super::LEVEL_MARGIN;

/// Holds a level's footprints as the deepest layer placed them, to resize its children without reordering them.
pub(super) struct SeedArrangement<'s> {
    seed: &'s BTreeMap<ElementPath, Rect>,
}

impl<'s> SeedArrangement<'s> {
    /// Arrange from the deepest layer's `seed` footprints of a ranked level, which never holds a pinned child.
    pub(super) fn new(seed: &'s BTreeMap<ElementPath, Rect>) -> Self {
        Self { seed }
    }

    /// Resize every child to `sizes`, then shift each one by what the children before it gained or freed, keeping every pair's order along each axis they are apart on.
    pub(super) fn rearranged(
        &self,
        sizes: &BTreeMap<ElementPath, Size>,
    ) -> BTreeMap<ElementPath, Rect> {
        let resized = self.resized_at_seed_corners(sizes);
        let shifted_along_x = self.shifted_along::<Horizontal>(&resized);
        self.shifted_along::<Vertical>(&shifted_along_x)
    }

    /// Resize every child to `sizes`, keeping its seed top-left corner.
    fn resized_at_seed_corners(
        &self,
        sizes: &BTreeMap<ElementPath, Size>,
    ) -> BTreeMap<ElementPath, Rect> {
        self.seed
            .iter()
            .map(|(path, seed_footprint)| {
                let seed_corner = seed_footprint.origin();
                let size = sizes[path];
                let resized = Rect::from_origin_size(seed_corner, size);
                (path.clone(), resized)
            })
            .collect()
    }

    /// Shift every footprint along `A`, visiting children in their seed order so each one sees its predecessors already shifted.
    fn shifted_along<A: Axis>(
        &self,
        footprints: &BTreeMap<ElementPath, Rect>,
    ) -> BTreeMap<ElementPath, Rect> {
        let mut seed_order: Vec<(&ElementPath, &Rect)> = self.seed.iter().collect();
        seed_order.sort_by(|(_left_path, left_seed), (_right_path, right_seed)| {
            let left_start = A::start(left_seed);
            let right_start = A::start(right_seed);
            left_start.total_cmp(&right_start)
        });
        let mut shifted = BTreeMap::new();
        for (path, seed_footprint) in seed_order {
            let shift = self.shift_keeping_seed_gaps_to_predecessors::<A>(seed_footprint, &shifted);
            let footprint = footprints[path];
            let shifted_footprint = A::shifted(footprint, shift);
            shifted.insert(path.clone(), shifted_footprint);
        }
        shifted
    }

    /// Compute how far a child moves along `A` to keep its seed gap to the predecessors directly facing it and stay after the others.
    // Δ_B = max(max_{A directly facing B}(x1'_A − x1_A) or 0 if none, max_{other A}(x1'_A + min(x0_B − x1_A, MIN_GAP) − x0_B))
    fn shift_keeping_seed_gaps_to_predecessors<A: Axis>(
        &self,
        seed_footprint: &Rect,
        shifted_predecessors: &BTreeMap<ElementPath, Rect>,
    ) -> f64 {
        let seed_start = A::start(seed_footprint);
        let predecessors: Vec<Predecessor> = shifted_predecessors
            .iter()
            .map(|(predecessor_path, predecessor_shifted)| {
                let predecessor_seed = self.seed[predecessor_path];
                Predecessor {
                    seed: predecessor_seed,
                    shifted: *predecessor_shifted,
                }
            })
            .filter(|predecessor| A::is_apart_before(&predecessor.seed, seed_footprint))
            .collect();
        let (facing, non_facing): (Vec<Predecessor>, Vec<Predecessor>) = predecessors
            .into_iter()
            .partition(|predecessor| A::is_facing(&predecessor.seed, seed_footprint));
        let facing_seeds: Vec<Rect> = facing.iter().map(|predecessor| predecessor.seed).collect();
        let (hidden, directly_facing): (Vec<Predecessor>, Vec<Predecessor>) = facing
            .into_iter()
            .partition(|predecessor| A::is_hidden_behind_any(&predecessor.seed, &facing_seeds));
        let facing_shift = directly_facing
            .iter()
            .map(|predecessor| {
                let shifted_end = A::end(&predecessor.shifted);
                let seed_end = A::end(&predecessor.seed);
                shifted_end - seed_end
            })
            .reduce(f64::max)
            .unwrap_or(0.0);
        let staying_after_shift = non_facing
            .iter()
            .chain(&hidden)
            .map(|predecessor| {
                let seed_end = A::end(&predecessor.seed);
                let seed_gap = seed_start - seed_end;
                let kept_gap = seed_gap.min(LEVEL_MARGIN);
                let shifted_end = A::end(&predecessor.shifted);
                shifted_end + kept_gap - seed_start
            })
            .fold(f64::NEG_INFINITY, f64::max);
        facing_shift.max(staying_after_shift)
    }
}

/// Pair a predecessor's seed footprint with its already shifted one.
#[derive(Clone, Copy)]
struct Predecessor {
    seed: Rect,
    shifted: Rect,
}

/// Read and shift footprints along one plan axis, so the x and y passes share one shifting rule.
trait Axis {
    type Across: Axis;

    fn start(footprint: &Rect) -> f64;

    fn end(footprint: &Rect) -> f64;

    fn shifted(footprint: Rect, shift: f64) -> Rect;

    /// Tell whether `before` ends no later than `after` starts along this axis.
    fn is_apart_before(before: &Rect, after: &Rect) -> bool {
        let before_end = Self::end(before);
        let after_start = Self::start(after);
        before_end <= after_start
    }

    /// Tell whether `hidden` faces one of the `closer` footprints that stands after it along this axis.
    fn is_hidden_behind_any(hidden: &Rect, closer: &[Rect]) -> bool {
        closer.iter().any(|closer_footprint| {
            let is_before_closer = Self::is_apart_before(hidden, closer_footprint);
            let is_facing_closer = Self::is_facing(hidden, closer_footprint);
            is_before_closer && is_facing_closer
        })
    }

    /// Tell whether two footprints overlap with positive length across this axis.
    fn is_facing(left: &Rect, right: &Rect) -> bool {
        let left_across_end = Self::Across::end(left);
        let right_across_end = Self::Across::end(right);
        let left_across_start = Self::Across::start(left);
        let right_across_start = Self::Across::start(right);
        let overlap_end = left_across_end.min(right_across_end);
        let overlap_start = left_across_start.max(right_across_start);
        overlap_end > overlap_start
    }
}

struct Horizontal;

struct Vertical;

impl Axis for Horizontal {
    type Across = Vertical;

    fn start(footprint: &Rect) -> f64 {
        footprint.x0
    }

    fn end(footprint: &Rect) -> f64 {
        footprint.x1
    }

    fn shifted(footprint: Rect, shift: f64) -> Rect {
        let offset = Vec2::new(shift, 0.0);
        footprint + offset
    }
}

impl Axis for Vertical {
    type Across = Horizontal;

    fn start(footprint: &Rect) -> f64 {
        footprint.y0
    }

    fn end(footprint: &Rect) -> f64 {
        footprint.y1
    }

    fn shifted(footprint: Rect, shift: f64) -> Rect {
        let offset = Vec2::new(0.0, shift);
        footprint + offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shrinking_child_frees_its_width_for_the_facing_row_behind_it_only() {
        let first = ElementPath::from_segments(["first"]);
        let middle = ElementPath::from_segments(["middle"]);
        let last = ElementPath::from_segments(["last"]);
        let far = ElementPath::from_segments(["far"]);
        let seed = BTreeMap::from([
            (first.clone(), Rect::new(0.0, 0.0, 100.0, 50.0)),
            (middle.clone(), Rect::new(118.0, 0.0, 218.0, 50.0)),
            (last.clone(), Rect::new(236.0, 0.0, 336.0, 50.0)),
            (far.clone(), Rect::new(400.0, 200.0, 450.0, 250.0)),
        ]);
        let arrangement = SeedArrangement::new(&seed);

        let seed_sizes: BTreeMap<ElementPath, Size> = seed
            .iter()
            .map(|(path, seed_footprint)| {
                let seed_size = seed_footprint.size();
                (path.clone(), seed_size)
            })
            .collect();
        let unchanged = arrangement.rearranged(&seed_sizes);
        assert_eq!(
            unchanged, seed,
            "with the seed sizes the output must equal the seed exactly"
        );

        let freed_width = 40.0;
        let mut closed_sizes = seed_sizes.clone();
        closed_sizes.insert(middle.clone(), Size::new(100.0 - freed_width, 50.0));
        let rearranged = arrangement.rearranged(&closed_sizes);

        let seed_last_x0 = seed[&last].x0;
        let last_x0 = rearranged[&last].x0;
        assert_eq!(
            last_x0,
            seed_last_x0 - freed_width,
            "a shrinking child must free exactly its lost width for the children facing it behind it"
        );
        let seed_middle_x1 = seed[&middle].x1;
        let middle_x1 = rearranged[&middle].x1;
        let seed_gap_middle_to_last = seed_last_x0 - seed_middle_x1;
        let gap_middle_to_last = last_x0 - middle_x1;
        assert_eq!(
            gap_middle_to_last, seed_gap_middle_to_last,
            "a child must keep its seed gap to the predecessor facing it"
        );
        let seed_far = seed[&far];
        let rearranged_far = rearranged[&far];
        assert_eq!(
            rearranged_far, seed_far,
            "a child with no facing predecessor must never move backward"
        );
        assert_no_two_footprints_overlap(&rearranged);
    }

    fn assert_no_two_footprints_overlap(footprints: &BTreeMap<ElementPath, Rect>) {
        for (left_path, left) in footprints {
            for (right_path, right) in footprints.range(left_path..).skip(1) {
                let overlap = left.intersect(*right);
                let overlap_area = overlap.area();
                assert!(
                    overlap_area <= 0.0,
                    "no two footprints may overlap: {left_path:?} and {right_path:?} do"
                );
            }
        }
    }
}
