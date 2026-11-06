use std::collections::BTreeMap;

use kurbo::{Point, Rect};
use svugy_model::model::PlacementHint;
use svugy_model::path::ElementPath;
use taffy::prelude::{TaffyMaxContent, auto, length, repeat};
use taffy::{Display, NodeId, Style, TaffyTree};

use super::{LevelLayout, SiblingGraph};

/// Gap between packed neighbours, the example's gap between the desk's modules.
const PACKED_GAP: f32 = 24.0;

/// Packs footprints over taffy into a grid of a fixed column count, filling rows in path order.
pub(super) struct Packed {
    column_count: u16,
}

impl Packed {
    /// Pack `child_count` children into `requested_columns`: c = clamp(max(min(requested, n), ⌈n / T⌉), 1, T), T the tracks taffy counts per axis.
    pub(super) fn in_columns(requested_columns: usize, child_count: usize) -> Self {
        let max_tracks = PlacementHint::MAX_PACKED_TRACKS as usize;
        let columns_with_a_child = requested_columns.min(child_count);
        let columns_keeping_rows_countable = child_count.div_ceil(max_tracks);
        let unclamped_column_count = columns_with_a_child.max(columns_keeping_rows_countable);
        let clamped_column_count = unclamped_column_count.clamp(1, max_tracks);
        let column_count = u16::try_from(clamped_column_count)
            .expect("the packed track limit fits taffy's u16 track count");
        Self { column_count }
    }
}

impl LevelLayout for Packed {
    fn place_footprints(&self, siblings: &SiblingGraph) -> BTreeMap<ElementPath, Rect> {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        let leaves: Vec<NodeId> = siblings
            .sizes()
            .values()
            .map(|size| {
                let leaf_width = length(size.width as f32);
                let leaf_height = length(size.height as f32);
                let leaf_size = taffy::Size {
                    width: leaf_width,
                    height: leaf_height,
                };
                let leaf_style = Style {
                    size: leaf_size,
                    ..Style::default()
                };
                tree.new_leaf(leaf_style).expect("taffy accepts a leaf")
            })
            .collect();
        let grid_style = self.grid_style();
        let grid = tree
            .new_with_children(grid_style, &leaves)
            .expect("taffy accepts the grid of its own leaves");
        tree.compute_layout(grid, taffy::Size::MAX_CONTENT)
            .expect("taffy lays out a grid of fixed-size leaves");
        siblings
            .sizes()
            .iter()
            .zip(&leaves)
            .map(|((path, &size), &leaf)| {
                let leaf_layout = tree.layout(leaf).expect("taffy laid out every leaf");
                let x = f64::from(leaf_layout.location.x);
                let y = f64::from(leaf_layout.location.y);
                let origin = Point::new(x, y);
                let footprint = Rect::from_origin_size(origin, size);
                (path.clone(), footprint)
            })
            .collect()
    }
}

impl Packed {
    fn grid_style(&self) -> Style {
        let content_sized_column = vec![auto()];
        let columns = repeat(self.column_count, content_sized_column);
        let gap = taffy::Size {
            width: length(PACKED_GAP),
            height: length(PACKED_GAP),
        };
        Style {
            display: Display::Grid,
            grid_template_columns: vec![columns],
            gap,
            ..Style::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use kurbo::Size;

    use super::*;

    fn siblings_of_size(child_count: usize, size: Size) -> SiblingGraph {
        let sizes = (0..child_count)
            .map(|child_index| {
                let path = ElementPath::from_segments([format!("child{child_index}")]);
                (path, size)
            })
            .collect();
        SiblingGraph {
            sizes,
            joined_pairs: BTreeSet::new(),
        }
    }

    #[test]
    fn column_count_never_exceeds_the_children_or_the_tracks_taffy_counts() {
        let max_tracks = PlacementHint::MAX_PACKED_TRACKS as usize;
        let cases = [
            (32_768, 3, 3),
            (usize::MAX, 3, 3),
            (0, 3, 1),
            (2, 0, 1),
            (usize::MAX, max_tracks + 5, max_tracks),
            (1, max_tracks + 5, 2),
        ];
        for (requested_columns, child_count, expected_column_count) in cases {
            let packed = Packed::in_columns(requested_columns, child_count);
            let column_count = usize::from(packed.column_count);
            assert_eq!(
                column_count, expected_column_count,
                "{requested_columns} for {child_count}"
            );
        }
    }

    #[test]
    fn a_grid_asking_for_more_columns_than_taffy_counts_packs_its_children_in_one_row() {
        let size = Size::new(40.0, 30.0);
        let siblings = siblings_of_size(3, size);
        let packed = Packed::in_columns(32_768, 3);

        let footprints = packed.place_footprints(&siblings);

        let tops: Vec<f64> = footprints.values().map(|footprint| footprint.y0).collect();
        assert_eq!(tops, [0.0, 0.0, 0.0]);
    }
}
