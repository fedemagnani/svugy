use std::collections::BTreeMap;

use kurbo::{Point, Rect, Size, Vec2};
use svugy_model::path::ElementPath;

use super::LEVEL_MARGIN;

/// Clearance free footprints keep from pinned ones, the level margin again.
const PINNED_CLEARANCE: f64 = LEVEL_MARGIN;

/// Holds footprints by path, to shift them together and bound them.
pub(super) struct Footprints {
    by_path: BTreeMap<ElementPath, Rect>,
}

impl Footprints {
    /// Hold the footprints keyed `by_path`.
    pub(super) fn new(by_path: BTreeMap<ElementPath, Rect>) -> Self {
        Self { by_path }
    }

    /// Translate these free footprints to the inset corner, or east of every pinned one if that corner crowds a pinned footprint.
    pub(super) fn shifted_clear_of_pinned(self, pinned: &Self, inset: f64) -> Self {
        let Some(free_union) = self.union() else {
            return self;
        };
        let inset_corner = Point::new(inset, inset);
        let shift_to_inset_corner = inset_corner - free_union.origin();
        let free_at_inset_corner = self.translated(shift_to_inset_corner);
        let Some(pinned_union) = pinned.union() else {
            return free_at_inset_corner;
        };
        let pinned_clearances: Vec<Rect> = pinned
            .by_path
            .values()
            .map(|footprint| footprint.inflate(PINNED_CLEARANCE, PINNED_CLEARANCE))
            .collect();
        let is_crowding_pinned = free_at_inset_corner.by_path.values().any(|footprint| {
            pinned_clearances
                .iter()
                .any(|clearance| clearance.overlaps(*footprint))
        });
        if is_crowding_pinned {
            let shift_east_of_pinned = Vec2::new(pinned_union.x1 + PINNED_CLEARANCE - inset, 0.0);
            return free_at_inset_corner.translated(shift_east_of_pinned);
        }
        free_at_inset_corner
    }

    /// Return these footprints joined with `others`, which win on a shared path.
    pub(super) fn merged_with(mut self, others: Self) -> Self {
        self.by_path.extend(others.by_path);
        self
    }

    /// Translate every footprint by shift = (max(0, inset − x0), max(0, inset − y0)) of their union, keeping pins already inside the inset exact.
    pub(super) fn shifted_inside_inset(self, inset: f64) -> Self {
        let Some(union) = self.union() else {
            return self;
        };
        let shift_x = (inset - union.x0).max(0.0);
        let shift_y = (inset - union.y0).max(0.0);
        let shift = Vec2::new(shift_x, shift_y);
        self.translated(shift)
    }

    /// Size the extent from the origin to the farthest footprint corner plus the inset.
    pub(super) fn extent_enclosing(&self, inset: f64) -> Size {
        let far_corner = self
            .by_path
            .values()
            .fold(Point::ORIGIN, |far_corner, footprint| {
                let x = far_corner.x.max(footprint.x1);
                let y = far_corner.y.max(footprint.y1);
                Point::new(x, y)
            });
        let width = far_corner.x + inset;
        let height = far_corner.y + inset;
        Size::new(width, height)
    }

    /// Return the union of every footprint, `None` when there is none.
    fn union(&self) -> Option<Rect> {
        self.by_path
            .values()
            .copied()
            .reduce(|union, footprint| union.union(footprint))
    }

    /// Return these footprints translated by `shift`.
    fn translated(self, shift: Vec2) -> Self {
        let by_path = self
            .by_path
            .into_iter()
            .map(|(path, footprint)| (path, footprint + shift))
            .collect();
        Self { by_path }
    }

    /// Return the footprints keyed by path.
    pub(super) fn into_by_path(self) -> BTreeMap<ElementPath, Rect> {
        self.by_path
    }
}
