//! Order one level's solids for the painter's algorithm.

use std::collections::BTreeMap;

use kurbo::Rect;
use svugy_model::path::ElementPath;

/// Orders disjoint ground footprints back to front, so nearer solids are painted over farther ones whatever their heights.
pub(super) struct DepthOrder<'a> {
    pending: Vec<(&'a ElementPath, Rect)>,
}

impl<'a> DepthOrder<'a> {
    /// Return every path of `footprints` back to front: each next one is the first in path order that no pending footprint must precede, else the first pending.
    pub(super) fn back_to_front(
        footprints: &'a BTreeMap<ElementPath, Rect>,
    ) -> Vec<&'a ElementPath> {
        let pending = footprints
            .iter()
            .map(|(path, footprint)| (path, *footprint))
            .collect();
        let mut depth_order = Self { pending };
        let footprint_count = footprints.len();
        let mut ordered = Vec::with_capacity(footprint_count);
        for _ordered_count in 0..footprint_count {
            let unobstructed_index = depth_order.first_unobstructed_index();
            let next_index = unobstructed_index.unwrap_or(0);
            let (path, _footprint) = depth_order.pending.remove(next_index);
            ordered.push(path);
        }
        ordered
    }

    /// Return the index of the first pending footprint no other pending one must be painted before.
    fn first_unobstructed_index(&self) -> Option<usize> {
        let mut indices = 0..self.pending.len();
        indices.find(|&candidate_index| self.is_unobstructed(candidate_index))
    }

    /// Tell whether no other pending footprint must be painted before the candidate.
    fn is_unobstructed(&self, candidate_index: usize) -> bool {
        let (_candidate_path, candidate) = self.pending[candidate_index];
        let mut other_footprints = self
            .pending
            .iter()
            .enumerate()
            .filter(|(other_index, _other)| *other_index != candidate_index);
        let has_pending_footprint_behind = other_footprints
            .any(|(_other_index, (_other_path, other))| Self::must_paint_before(*other, candidate));
        !has_pending_footprint_behind
    }

    /// Tell whether some sight line, a ground line x − y = k, meets `behind` before `front`: their x − y spans overlap and `behind` lies wholly at lower x or lower y.
    pub(super) fn must_paint_before(behind: Rect, front: Rect) -> bool {
        let behind_starts_before_front_ends = behind.x0 - behind.y1 < front.x1 - front.y0;
        let front_starts_before_behind_ends = front.x0 - front.y1 < behind.x1 - behind.y0;
        let is_screen_x_overlap =
            behind_starts_before_front_ends && front_starts_before_behind_ends;
        let is_wholly_at_lower_x = behind.x1 <= front.x0;
        let is_wholly_at_lower_y = behind.y1 <= front.y0;
        let is_wholly_farther = is_wholly_at_lower_x || is_wholly_at_lower_y;
        is_screen_x_overlap && is_wholly_farther
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn footprints(rects: &[(&str, Rect)]) -> BTreeMap<ElementPath, Rect> {
        rects
            .iter()
            .map(|(name, rect)| (ElementPath::from_segments([*name]), *rect))
            .collect()
    }

    fn names(order: &[&ElementPath]) -> Vec<String> {
        order.iter().map(|path| path.to_string()).collect()
    }

    fn assert_no_footprint_painted_over_a_nearer_one(footprints: &BTreeMap<ElementPath, Rect>) {
        let order = DepthOrder::back_to_front(footprints);
        assert_eq!(order.len(), footprints.len());
        for (earlier_index, earlier) in order.iter().enumerate() {
            for later in &order[earlier_index + 1..] {
                let is_order_inverted =
                    DepthOrder::must_paint_before(footprints[*later], footprints[*earlier]);
                assert!(
                    !is_order_inverted,
                    "{later} must be painted before {earlier}"
                );
            }
        }
    }

    #[test]
    fn long_box_behind_a_small_one_is_painted_first_though_its_centre_is_nearer() {
        let set = footprints(&[
            ("a_small_front", Rect::new(10.0, 10.0, 20.0, 20.0)),
            ("z_long_back", Rect::new(0.0, 0.0, 100.0, 10.0)),
        ]);
        let order = DepthOrder::back_to_front(&set);
        assert_eq!(names(&order), ["z_long_back", "a_small_front"]);
        assert_no_footprint_painted_over_a_nearer_one(&set);
    }

    #[test]
    fn long_box_in_front_of_a_small_one_is_painted_last_though_its_corner_is_farther() {
        let set = footprints(&[
            ("a_long_front", Rect::new(0.0, 10.0, 100.0, 20.0)),
            ("z_small_back", Rect::new(50.0, 0.0, 60.0, 10.0)),
        ]);
        let order = DepthOrder::back_to_front(&set);
        assert_eq!(names(&order), ["z_small_back", "a_long_front"]);
        assert_no_footprint_painted_over_a_nearer_one(&set);
    }

    #[test]
    fn pinwheel_of_long_thin_boxes_gets_a_consistent_order() {
        let set = footprints(&[
            ("north", Rect::new(0.0, 0.0, 20.0, 10.0)),
            ("east", Rect::new(20.0, 0.0, 30.0, 20.0)),
            ("south", Rect::new(10.0, 20.0, 30.0, 30.0)),
            ("west", Rect::new(0.0, 10.0, 10.0, 30.0)),
            ("hub", Rect::new(10.0, 10.0, 20.0, 20.0)),
        ]);
        assert_no_footprint_painted_over_a_nearer_one(&set);
    }

    #[test]
    fn staircase_of_long_boxes_is_ordered_along_every_sight_line() {
        let set = footprints(&[
            ("a", Rect::new(0.0, 0.0, 200.0, 8.0)),
            ("b", Rect::new(190.0, 8.0, 198.0, 300.0)),
            ("c", Rect::new(0.0, 8.0, 180.0, 16.0)),
            ("d", Rect::new(-40.0, 30.0, -30.0, 400.0)),
            ("e", Rect::new(-30.0, 300.0, 400.0, 310.0)),
            ("f", Rect::new(20.0, 40.0, 60.0, 80.0)),
            ("g", Rect::new(60.0, 20.0, 140.0, 30.0)),
        ]);
        assert_no_footprint_painted_over_a_nearer_one(&set);
    }

    #[test]
    fn footprints_apart_on_screen_impose_no_order() {
        let back_left = Rect::new(0.0, 100.0, 10.0, 110.0);
        let back_right = Rect::new(100.0, 0.0, 110.0, 10.0);
        let is_left_first = DepthOrder::must_paint_before(back_left, back_right);
        let is_right_first = DepthOrder::must_paint_before(back_right, back_left);
        assert!(!is_left_first);
        assert!(!is_right_first);
    }

    #[test]
    fn grid_of_boxes_orders_identically_on_every_run() {
        let rects: Vec<(String, Rect)> = (0..6)
            .flat_map(|row| (0..6).map(move |column| (row, column)))
            .map(|(row, column)| {
                let x0 = f64::from(column) * 30.0;
                let y0 = f64::from(row) * 25.0;
                let name = format!("n{row}{column}");
                (name, Rect::new(x0, y0, x0 + 20.0, y0 + 15.0))
            })
            .collect();
        let borrowed: Vec<(&str, Rect)> = rects
            .iter()
            .map(|(name, rect)| (name.as_str(), *rect))
            .collect();
        let set = footprints(&borrowed);
        let first_run = names(&DepthOrder::back_to_front(&set));
        let second_run = names(&DepthOrder::back_to_front(&set));
        assert_eq!(first_run, second_run);
        assert_no_footprint_painted_over_a_nearer_one(&set);
    }
}
