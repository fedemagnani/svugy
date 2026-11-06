//! Place one level's children as footprints: pinned ones where the author put them, free ones automatically around them.

mod layered;
mod packed;

use std::collections::{BTreeMap, BTreeSet};

use kurbo::{Point, Rect, Size, Vec2};
use svugy_model::model::{Element, Model, PlacementHint};
use svugy_model::path::ElementPath;

use super::{LevelPlan, LevelPlans};
use crate::metrics::{LabelBlock, LabelRole, TextMeasure};
use layered::Layered;
use packed::Packed;

/// Clear margin between a level's boundary and its outermost footprints, the example's inset inside the desk.
const LEVEL_MARGIN: f64 = 18.0;
/// Extra margin a level left by edges reserves on every side for its stub markers, as deep as the level margin.
pub(super) const STUB_MARGIN: f64 = 18.0;
/// Clearance free footprints keep from pinned ones, the level margin again.
const PINNED_CLEARANCE: f64 = 18.0;
/// Smallest footprint, the example's smallest block (`Privy`).
const MIN_FOOTPRINT: Size = Size::new(72.0, 66.0);

/// Places one level's footprints, from their sizes and the pairs edges join, in a frame of its own choosing.
pub(super) trait LevelLayout {
    fn place_footprints(&self, siblings: &SiblingGraph) -> BTreeMap<ElementPath, Rect>;
}

/// Holds the footprint size of every child of one level and the pairs of children an edge joins, directly or lifted.
pub(super) struct SiblingGraph {
    sizes: BTreeMap<ElementPath, Size>,
    joined_pairs: BTreeSet<(ElementPath, ElementPath)>,
}

impl SiblingGraph {
    /// Size every child of `parent`, a container by its own level's extent and a leaf by its labels, with the pairs edges join.
    pub(super) fn measure_children(
        model: &Model,
        parent: &ElementPath,
        joined_pairs: BTreeSet<(ElementPath, ElementPath)>,
        interiors: &LevelPlans,
        measure: &impl TextMeasure,
    ) -> Self {
        let sizes = model
            .children(parent)
            .map(|(path, element)| {
                let interior = interiors.level(path);
                let size = interior.map_or_else(
                    || Self::footprint_size_fitting_labels(element, measure),
                    |interior_plan| interior_plan.extent,
                );
                (path.clone(), size)
            })
            .collect();
        Self {
            sizes,
            joined_pairs,
        }
    }

    /// Size a footprint to fit the room its title over its caption needs on its top face, never below the smallest block.
    fn footprint_size_fitting_labels(element: &Element, measure: &impl TextMeasure) -> Size {
        let label_block = LabelBlock::measure(element, LabelRole::Title, measure);
        let label_room = label_block.top_face_room();
        let width = label_room.width.max(MIN_FOOTPRINT.width);
        let depth = label_room.height.max(MIN_FOOTPRINT.height);
        Size::new(width, depth)
    }

    /// Split off the children the author pinned, as footprints at their positions, keeping the free ones and the pairs they join.
    fn split_off_pinned(self, model: &Model) -> (BTreeMap<ElementPath, Rect>, Self) {
        let mut pinned_footprints = BTreeMap::new();
        let mut free_sizes = BTreeMap::new();
        for (path, size) in self.sizes {
            let element = model.element(&path);
            let position = element.and_then(Element::position);
            let Some(position) = position else {
                free_sizes.insert(path, size);
                continue;
            };
            let pinned_origin = Point::new(position.x, position.y);
            let pinned_footprint = Rect::from_origin_size(pinned_origin, size);
            pinned_footprints.insert(path, pinned_footprint);
        }
        let free_joined_pairs = self
            .joined_pairs
            .into_iter()
            .filter(|(from, to)| {
                let is_from_free = free_sizes.contains_key(from);
                let is_to_free = free_sizes.contains_key(to);
                is_from_free && is_to_free
            })
            .collect();
        let free_siblings = Self {
            sizes: free_sizes,
            joined_pairs: free_joined_pairs,
        };
        (pinned_footprints, free_siblings)
    }

    /// Place the footprints with the strategy `hint` names; no hint, as at the root, means layered.
    fn place_by_hint(&self, hint: Option<PlacementHint>) -> BTreeMap<ElementPath, Rect> {
        let child_count = self.sizes.len();
        match hint {
            None | Some(PlacementHint::Layered) => Layered.place_footprints(self),
            Some(PlacementHint::Grid { columns }) => {
                let requested_columns = usize::try_from(columns).unwrap_or(usize::MAX);
                Packed::in_columns(requested_columns, child_count).place_footprints(self)
            }
            Some(PlacementHint::Row) => {
                Packed::in_columns(child_count, child_count).place_footprints(self)
            }
            Some(PlacementHint::Column) => {
                Packed::in_columns(1, child_count).place_footprints(self)
            }
        }
    }

    pub(super) fn sizes(&self) -> &BTreeMap<ElementPath, Size> {
        &self.sizes
    }

    pub(super) fn joined_pairs(&self) -> &BTreeSet<(ElementPath, ElementPath)> {
        &self.joined_pairs
    }
}

impl LevelPlan {
    /// Place the children of `parent` around the pinned ones by its hint, inside a margin with stub room when edges leave it.
    pub(super) fn place_children(
        model: &Model,
        parent: &ElementPath,
        siblings: SiblingGraph,
        is_left_by_edges: bool,
        measure: &impl TextMeasure,
    ) -> Self {
        let inset = Self::inset_inside_boundary(is_left_by_edges);
        let (pinned_footprints, free_siblings) = siblings.split_off_pinned(model);
        let parent_element = model.element(parent);
        let hint = parent_element.and_then(Element::child_placement);
        let free_footprints = free_siblings.place_by_hint(hint);
        let cleared_free_footprints =
            Self::shift_clear_of_pinned(free_footprints, &pinned_footprints, inset);
        let mut footprints = pinned_footprints;
        footprints.extend(cleared_free_footprints);
        let inset_footprints = Self::shift_inside_inset(footprints, inset);
        let enclosing_extent = Self::extent_enclosing(&inset_footprints, inset);
        let hanging_labels_width = parent_element.map_or(0.0, |element| {
            let tag_block = LabelBlock::measure(element, LabelRole::ContainerTag, measure);
            tag_block.hanging_width()
        });
        let width = enclosing_extent.width.max(hanging_labels_width);
        let extent = Size::new(width, enclosing_extent.height);
        let mut plan = Self::new(extent);
        plan.footprints = inset_footprints;
        plan
    }

    /// Return the margin kept inside the boundary: the level margin, plus the stub margin when edges leave the level.
    fn inset_inside_boundary(is_left_by_edges: bool) -> f64 {
        if is_left_by_edges {
            return LEVEL_MARGIN + STUB_MARGIN;
        }
        LEVEL_MARGIN
    }

    /// Translate the free footprints to the inset corner, or east of every pinned one if that corner crowds a pinned footprint.
    fn shift_clear_of_pinned(
        free_footprints: BTreeMap<ElementPath, Rect>,
        pinned_footprints: &BTreeMap<ElementPath, Rect>,
        inset: f64,
    ) -> BTreeMap<ElementPath, Rect> {
        let Some(free_union) = Self::union_of(&free_footprints) else {
            return free_footprints;
        };
        let inset_corner = Point::new(inset, inset);
        let shift_to_inset_corner = inset_corner - free_union.origin();
        let free_at_inset_corner = Self::translated(free_footprints, shift_to_inset_corner);
        let Some(pinned_union) = Self::union_of(pinned_footprints) else {
            return free_at_inset_corner;
        };
        let pinned_clearances: Vec<Rect> = pinned_footprints
            .values()
            .map(|footprint| footprint.inflate(PINNED_CLEARANCE, PINNED_CLEARANCE))
            .collect();
        let is_crowding_pinned = free_at_inset_corner.values().any(|footprint| {
            pinned_clearances
                .iter()
                .any(|clearance| clearance.overlaps(*footprint))
        });
        if is_crowding_pinned {
            let shift_east_of_pinned = Vec2::new(pinned_union.x1 + PINNED_CLEARANCE - inset, 0.0);
            return Self::translated(free_at_inset_corner, shift_east_of_pinned);
        }
        free_at_inset_corner
    }

    /// Translate every footprint by what brings their union's top-left corner to the inset, keeping pins already inside it exact.
    fn shift_inside_inset(
        footprints: BTreeMap<ElementPath, Rect>,
        inset: f64,
    ) -> BTreeMap<ElementPath, Rect> {
        let Some(union) = Self::union_of(&footprints) else {
            return footprints;
        };
        let shift_x = (inset - union.x0).max(0.0);
        let shift_y = (inset - union.y0).max(0.0);
        let shift = Vec2::new(shift_x, shift_y);
        Self::translated(footprints, shift)
    }

    /// Size the level from its origin to the farthest footprint corner plus the inset.
    fn extent_enclosing(footprints: &BTreeMap<ElementPath, Rect>, inset: f64) -> Size {
        let inset_offset = Vec2::new(inset, inset);
        let far_corner = footprints
            .values()
            .fold(Point::ORIGIN, |far_corner, footprint| {
                let x = far_corner.x.max(footprint.x1);
                let y = far_corner.y.max(footprint.y1);
                Point::new(x, y)
            });
        let far_corner_with_inset = far_corner + inset_offset;
        far_corner_with_inset.to_vec2().to_size()
    }

    fn union_of(footprints: &BTreeMap<ElementPath, Rect>) -> Option<Rect> {
        footprints
            .values()
            .copied()
            .reduce(|union, footprint| union.union(footprint))
    }

    fn translated(
        footprints: BTreeMap<ElementPath, Rect>,
        shift: Vec2,
    ) -> BTreeMap<ElementPath, Rect> {
        footprints
            .into_iter()
            .map(|(path, footprint)| (path, footprint + shift))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use svugy_model::builder::ModelBuilder;
    use svugy_model::model::{Edge, Position};

    use super::*;
    use crate::metrics::BundledFont;

    fn place_level(model: &Model, level: &ElementPath, is_left_by_edges: bool) -> LevelPlan {
        let measure = BundledFont::new();
        let joined_pairs = model
            .layoutable_edges()
            .map(|(edge_id, _edge)| (edge_id.from().clone(), edge_id.to().clone()))
            .collect();
        let no_interiors = LevelPlans::new(BTreeMap::new());
        let siblings =
            SiblingGraph::measure_children(model, level, joined_pairs, &no_interiors, &measure);
        LevelPlan::place_children(model, level, siblings, is_left_by_edges, &measure)
    }

    fn place_top_level(model: &Model) -> LevelPlan {
        let root = ElementPath::root();
        place_level(model, &root, false)
    }

    fn assert_no_two_footprints_overlap(plan: &LevelPlan) {
        let footprints: Vec<(&ElementPath, &Rect)> = plan.footprints.iter().collect();
        for (index, (path, footprint)) in footprints.iter().enumerate() {
            for (other_path, other_footprint) in &footprints[index + 1..] {
                let is_overlapping = footprint.overlaps(**other_footprint);
                assert!(!is_overlapping, "{path} overlaps {other_path}");
            }
        }
    }

    fn assert_every_footprint_keeps_the_margin_inside_the_boundary(plan: &LevelPlan, margin: f64) {
        let inner_boundary = plan.boundary().inset(-margin);
        for (path, footprint) in &plan.footprints {
            let is_inside_margin = inner_boundary.contains_rect(*footprint);
            assert!(is_inside_margin, "{path} intrudes into the margin");
        }
    }

    #[test]
    fn longer_labels_give_wider_footprints() {
        let mut builder = ModelBuilder::new();
        builder.add_top_level("short", Element::new("API"));
        builder.add_top_level("long", Element::new("Payment ledger reconciliation"));
        let long_caption = Element::new("API").with_caption("reads every ledger entry twice a day");
        builder.add_top_level("captioned", long_caption);
        let model = builder.build().model.expect("fixture model is valid");

        let plan = place_top_level(&model);

        let short_width = plan.footprints[&ElementPath::from_segments(["short"])].width();
        let long_width = plan.footprints[&ElementPath::from_segments(["long"])].width();
        let captioned_width = plan.footprints[&ElementPath::from_segments(["captioned"])].width();
        assert_eq!(short_width, MIN_FOOTPRINT.width);
        assert!(long_width > short_width);
        assert!(captioned_width > short_width);
    }

    #[test]
    fn layered_footprints_never_overlap() {
        let mut builder = ModelBuilder::new();
        let names = [
            "app", "api", "auth", "queue", "worker", "db", "cache", "mail",
        ];
        let elements: Vec<_> = names
            .iter()
            .map(|name| builder.add_top_level(name, Element::new(*name)))
            .collect();
        let edge_pairs = [
            (0, 1),
            (0, 2),
            (1, 3),
            (2, 5),
            (3, 4),
            (4, 5),
            (1, 6),
            (4, 7),
            (2, 6),
        ];
        for (from_index, to_index) in edge_pairs {
            builder.add_edge(&elements[from_index], &elements[to_index], Edge::new());
        }
        let model = builder.build().model.expect("fixture model is valid");

        let plan = place_top_level(&model);

        assert_eq!(plan.footprints.len(), names.len());
        assert_no_two_footprints_overlap(&plan);
        assert_every_footprint_keeps_the_margin_inside_the_boundary(&plan, LEVEL_MARGIN);
    }

    #[test]
    fn explicit_positions_override_automatic_placement() {
        let mut builder = ModelBuilder::new();
        let pinned_position = Position { x: 400.0, y: 300.0 };
        let pinned = Element::new("pinned").with_position(pinned_position);
        let pinned_ref = builder.add_top_level("pinned", pinned);
        let free_ref = builder.add_top_level("free", Element::new("free"));
        builder.add_edge(&free_ref, &pinned_ref, Edge::new());
        let model = builder.build().model.expect("fixture model is valid");

        let plan = place_top_level(&model);

        let pinned_footprint = plan.footprints[&ElementPath::from_segments(["pinned"])];
        assert_eq!(pinned_footprint.origin(), Point::new(400.0, 300.0));
        assert!(plan.boundary().contains_rect(pinned_footprint));
    }

    #[test]
    fn a_free_sibling_never_overlaps_a_footprint_pinned_at_the_margin_corner() {
        let mut builder = ModelBuilder::new();
        let pinned_position = Position { x: 18.0, y: 18.0 };
        let pinned = Element::new("pinned").with_position(pinned_position);
        builder.add_top_level("pinned", pinned);
        builder.add_top_level("free", Element::new("free"));
        let model = builder.build().model.expect("fixture model is valid");

        let plan = place_top_level(&model);

        let pinned_footprint = plan.footprints[&ElementPath::from_segments(["pinned"])];
        let pinned_origin = Point::new(pinned_position.x, pinned_position.y);
        assert_eq!(pinned_footprint.origin(), pinned_origin);
        assert_no_two_footprints_overlap(&plan);
        assert_every_footprint_keeps_the_margin_inside_the_boundary(&plan, LEVEL_MARGIN);
    }

    #[test]
    fn positions_inside_the_margin_shift_together_until_the_level_encloses_them() {
        let mut builder = ModelBuilder::new();
        let west_position = Position { x: -50.0, y: 100.0 };
        builder.add_top_level("west", Element::new("west").with_position(west_position));
        let north_position = Position { x: 200.0, y: 0.0 };
        builder.add_top_level("north", Element::new("north").with_position(north_position));
        let model = builder.build().model.expect("fixture model is valid");

        let plan = place_top_level(&model);

        let west = plan.footprints[&ElementPath::from_segments(["west"])];
        let north = plan.footprints[&ElementPath::from_segments(["north"])];
        let west_origin_inside_margin = Point::new(LEVEL_MARGIN, 100.0 + LEVEL_MARGIN);
        assert_eq!(west.origin(), west_origin_inside_margin);
        assert_eq!(north.x0 - west.x0, 250.0);
        assert_every_footprint_keeps_the_margin_inside_the_boundary(&plan, LEVEL_MARGIN);
    }

    #[test]
    fn a_level_left_by_edges_reserves_stub_room_on_every_side() {
        let mut builder = ModelBuilder::new();
        builder.add_top_level("a", Element::new("a"));
        let model = builder.build().model.expect("fixture model is valid");
        let root = ElementPath::root();

        let plan = place_level(&model, &root, true);

        let stubbed_margin = LEVEL_MARGIN + STUB_MARGIN;
        let footprint = plan.footprints[&ElementPath::from_segments(["a"])];
        let stubbed_margin_corner = Point::new(stubbed_margin, stubbed_margin);
        assert_eq!(footprint.origin(), stubbed_margin_corner);
        assert_eq!(plan.extent.width - footprint.x1, stubbed_margin);
        assert_eq!(plan.extent.height - footprint.y1, stubbed_margin);
    }

    #[test]
    fn a_container_level_is_at_least_as_wide_as_its_hanging_tag_and_caption() {
        let mut builder = ModelBuilder::new();
        let long_tag = "settlement and reconciliation services";
        let cluster = builder.add_top_level("cluster", Element::new(long_tag));
        builder.add_child(&cluster, "x", Element::new("x"));
        let model = builder.build().model.expect("fixture model is valid");
        let measure = BundledFont::new();
        let cluster_element = model.element(cluster.path()).expect("cluster is declared");
        let tag_block = LabelBlock::measure(cluster_element, LabelRole::ContainerTag, &measure);

        let plan = place_level(&model, cluster.path(), false);

        assert_eq!(plan.extent.width, tag_block.hanging_width());
        let only_footprint = plan.footprints[&cluster.path().child("x")];
        assert!(plan.extent.width > only_footprint.x1 + LEVEL_MARGIN);
    }

    #[test]
    fn grid_hint_packs_children_into_rows_of_its_column_count() {
        let mut builder = ModelBuilder::new();
        let grid = Element::new("cluster").with_child_placement(PlacementHint::Grid { columns: 2 });
        let cluster = builder.add_top_level("cluster", grid);
        for name in ["a", "b", "c", "d", "e"] {
            builder.add_child(&cluster, name, Element::new(name));
        }
        let model = builder.build().model.expect("fixture model is valid");

        let plan = place_level(&model, cluster.path(), false);

        let footprint_of = |name: &str| plan.footprints[&cluster.path().child(name)];
        let (a, b, c, e) = (
            footprint_of("a"),
            footprint_of("b"),
            footprint_of("c"),
            footprint_of("e"),
        );
        assert_eq!(a.y0, b.y0);
        assert!(b.x0 > a.x1);
        assert_eq!(c.x0, a.x0);
        assert!(c.y0 > a.y1);
        assert!(e.y0 > c.y1);
        assert_no_two_footprints_overlap(&plan);
    }

    #[test]
    fn row_and_column_hints_line_children_up_along_one_axis() {
        let mut builder = ModelBuilder::new();
        let row = builder.add_top_level(
            "row",
            Element::new("row").with_child_placement(PlacementHint::Row),
        );
        let column = builder.add_top_level(
            "column",
            Element::new("column").with_child_placement(PlacementHint::Column),
        );
        for name in ["a", "b", "c"] {
            builder.add_child(&row, name, Element::new(name));
            builder.add_child(&column, name, Element::new(name));
        }
        let model = builder.build().model.expect("fixture model is valid");

        let row_plan = place_level(&model, row.path(), false);
        let column_plan = place_level(&model, column.path(), false);

        let row_tops: Vec<f64> = row_plan
            .footprints
            .values()
            .map(|footprint| footprint.y0)
            .collect();
        let column_lefts: Vec<f64> = column_plan
            .footprints
            .values()
            .map(|footprint| footprint.x0)
            .collect();
        assert!(row_tops.iter().all(|&top| top == row_tops[0]));
        assert!(column_lefts.iter().all(|&left| left == column_lefts[0]));
        assert_no_two_footprints_overlap(&row_plan);
        assert_no_two_footprints_overlap(&column_plan);
    }
}
