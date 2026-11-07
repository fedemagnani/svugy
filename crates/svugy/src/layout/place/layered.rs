use std::collections::BTreeMap;

use kurbo::{Point, Rect};
use svugy_dagre::LeftToRightLayout;
use svugy_model::path::ElementPath;

use super::{LevelLayout, SiblingGraph};

/// Gap between neighbours within one rank, twice the example's gap between a container's packed children.
const NODE_GAP: f64 = 48.0;
/// Gap between consecutive ranks, wider than the node gap so the arcs between ranks have room.
const RANK_GAP: f64 = 72.0;
/// Ranks advance along plan x, so a request path reads from the back left toward the front right.
const LEFT_TO_RIGHT_LAYOUT: LeftToRightLayout = LeftToRightLayout {
    node_gap: NODE_GAP,
    rank_gap: RANK_GAP,
};

/// Places footprints in dagre ranks along plan x, so edges flow in one direction.
pub(super) struct Layered;

impl LevelLayout for Layered {
    fn place_footprints(&self, siblings: &SiblingGraph) -> BTreeMap<ElementPath, Rect> {
        let sizes = siblings.sizes();
        let node_index_by_path: BTreeMap<&ElementPath, usize> = sizes
            .keys()
            .enumerate()
            .map(|(node_index, path)| (path, node_index))
            .collect();
        let node_sizes: Vec<svugy_dagre::Size> = sizes
            .values()
            .map(|size| svugy_dagre::Size {
                width: size.width,
                height: size.height,
            })
            .collect();
        let edges: Vec<(usize, usize)> = siblings
            .joined_pairs()
            .iter()
            .map(|(from, to)| {
                let from_index = node_index_by_path[from];
                let to_index = node_index_by_path[to];
                (from_index, to_index)
            })
            .collect();
        let centres = LEFT_TO_RIGHT_LAYOUT.node_centres(&node_sizes, &edges);
        sizes
            .iter()
            .zip(centres)
            .map(|((path, &size), centre)| {
                let center = Point::new(centre.x, centre.y);
                let footprint = Rect::from_center_size(center, size);
                let path_key = path.clone();
                (path_key, footprint)
            })
            .collect()
    }
}
