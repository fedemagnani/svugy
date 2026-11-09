//! Move children within their rank bands after a ranked placement, as an arrangement picks: to either end of the band or past their south neighbour, then clear the overlaps a move leaves.

use std::collections::BTreeMap;

use kurbo::{Rect, Vec2};
use svugy_model::model::Model;
use svugy_model::path::ElementPath;

use super::footprints::Footprints;
use super::{LEVEL_MARGIN, RankedPlacement};
use crate::layout::arrangements::{Arrangements, RankMove};
use crate::layout::edge_levels::LeavingEdges;
use crate::layout::{PlacedLevel, PlacedLevels};
use crate::metrics::TextMeasure;

/// Gap a move keeps between band neighbours it brings together, the compound layout's gap within a rank.
const BAND_GAP: f64 = 48.0;

/// Places ranked levels with `inner`, then moves the children of the level it places as a region root within their rank bands, as `arrangements` picks.
pub(in crate::layout) struct MovedWithinRanks<'p, P> {
    inner: &'p P,
    arrangements: &'p Arrangements,
}

impl<'p, P> MovedWithinRanks<'p, P> {
    /// Place with `inner`, moving children as `arrangements` picks.
    pub(in crate::layout) fn new(inner: &'p P, arrangements: &'p Arrangements) -> Self {
        Self {
            inner,
            arrangements,
        }
    }
}

impl<P: RankedPlacement> RankedPlacement for MovedWithinRanks<'_, P> {
    /// Place `level` with the inner placement, then apply its rank moves to it alone: the containers placed jointly with it keep their extents.
    fn place_ranked_level(
        &self,
        model: &Model,
        level: &ElementPath,
        leaving_edges: &LeavingEdges,
        interiors: &PlacedLevels,
        measure: &impl TextMeasure,
    ) -> Vec<(ElementPath, PlacedLevel)> {
        let mut placed_levels =
            self.inner
                .place_ranked_level(model, level, leaving_edges, interiors, measure);
        let rank_moves = self.arrangements.rank_moves_of(level);
        let has_no_move = rank_moves.is_empty();
        if has_no_move {
            return placed_levels;
        }
        let is_left_by_edges = leaving_edges.is_left_by_edges(level);
        for (placed_path, placed_level) in &mut placed_levels {
            let is_moved_level = placed_path == level;
            if !is_moved_level {
                continue;
            }
            let moved_level =
                placed_level.moved_within_ranks(model, level, rank_moves, is_left_by_edges, measure);
            *placed_level = moved_level;
        }
        placed_levels
    }
}

impl PlacedLevel {
    /// Apply `rank_moves` in order to the footprints of `level`'s children, clear the overlaps they leave keeping their new order, and refit the level's margin and extent.
    fn moved_within_ranks(
        &self,
        model: &Model,
        level: &ElementPath,
        rank_moves: &[RankMove],
        is_left_by_edges: bool,
        measure: &impl TextMeasure,
    ) -> Self {
        let mut footprints = self.footprints.clone();
        for rank_move in rank_moves {
            rank_move.apply_to(&mut footprints);
        }
        let cleared_by_path = Self::cleared_of_overlaps(footprints);
        let cleared_footprints = Footprints::new(cleared_by_path);
        let inset = Self::inset_inside_boundary(is_left_by_edges);
        let inset_footprints = cleared_footprints.shifted_onto_inset(inset);
        let level_element = model.element(level);
        let extent =
            Self::extent_fitting_footprints_and_tag(&inset_footprints, inset, level_element, measure);
        let footprints_by_path = inset_footprints.into_by_path();
        Self::new(extent, footprints_by_path)
    }

    /// Push footprints south, in order of their north edges, until none comes within the level margin of another it shares plan x with, each pushed one landing a band gap past the one it cleared.
    fn cleared_of_overlaps(
        footprints: BTreeMap<ElementPath, Rect>,
    ) -> BTreeMap<ElementPath, Rect> {
        let mut north_first: Vec<(ElementPath, Rect)> = footprints.into_iter().collect();
        north_first.sort_by(|(path, footprint), (other_path, other_footprint)| {
            footprint
                .y0
                .total_cmp(&other_footprint.y0)
                .then_with(|| path.cmp(other_path))
        });
        let mut cleared: Vec<(ElementPath, Rect)> = Vec::new();
        for (path, footprint) in north_first {
            let mut pushed_footprint = footprint;
            let mut is_pushed = true;
            while is_pushed {
                is_pushed = false;
                for (_cleared_path, cleared_footprint) in &cleared {
                    let is_too_close = Self::is_within_margin(pushed_footprint, *cleared_footprint);
                    if !is_too_close {
                        continue;
                    }
                    let push = cleared_footprint.y1 + BAND_GAP - pushed_footprint.y0;
                    let push_offset = Vec2::new(0.0, push);
                    pushed_footprint = pushed_footprint + push_offset;
                    is_pushed = true;
                }
            }
            cleared.push((path, pushed_footprint));
        }
        cleared.into_iter().collect()
    }

    /// Tell whether `footprint` overlaps `other` along plan x and comes within the level margin of it along plan y.
    fn is_within_margin(footprint: Rect, other: Rect) -> bool {
        let is_sharing_x = footprint.x0 < other.x1 && footprint.x1 > other.x0;
        let is_near_along_y =
            footprint.y0 < other.y1 + LEVEL_MARGIN && footprint.y1 + LEVEL_MARGIN > other.y0;
        is_sharing_x && is_near_along_y
    }
}

impl RankMove {
    /// Move this move's child in `footprints`, leaving them unchanged when its band holds no sibling to move past.
    fn apply_to(&self, footprints: &mut BTreeMap<ElementPath, Rect>) {
        match self {
            Self::ToNorthEnd(child) => Self::move_to_north_end(footprints, child),
            Self::ToSouthEnd(child) => Self::move_to_south_end(footprints, child),
            Self::SwapWithSouthNeighbour(child) => Self::swap_with_south_neighbour(footprints, child),
        }
    }

    /// Move `child` a band gap north of every sibling sharing its plan x-range, unless it already stands north of them all.
    fn move_to_north_end(footprints: &mut BTreeMap<ElementPath, Rect>, child: &ElementPath) {
        let Some(&footprint) = footprints.get(child) else {
            return;
        };
        let band = Self::band_of(footprints, child, footprint);
        let band_north = band.iter().map(|(_path, sibling)| sibling.y0).reduce(f64::min);
        let Some(band_north) = band_north else {
            return;
        };
        let is_north_end = footprint.y1 <= band_north;
        if is_north_end {
            return;
        }
        let moved_north = band_north - BAND_GAP - footprint.height();
        Self::move_north_edge_to(footprints, child, footprint, moved_north);
    }

    /// Move `child` a band gap south of every sibling sharing its plan x-range, unless it already stands south of them all.
    fn move_to_south_end(footprints: &mut BTreeMap<ElementPath, Rect>, child: &ElementPath) {
        let Some(&footprint) = footprints.get(child) else {
            return;
        };
        let band = Self::band_of(footprints, child, footprint);
        let band_south = band.iter().map(|(_path, sibling)| sibling.y1).reduce(f64::max);
        let Some(band_south) = band_south else {
            return;
        };
        let is_south_end = footprint.y0 >= band_south;
        if is_south_end {
            return;
        }
        let moved_north = band_south + BAND_GAP;
        Self::move_north_edge_to(footprints, child, footprint, moved_north);
    }

    /// Swap `child` with the nearest sibling south of it sharing its plan x-range, keeping the gap between them and the span they cover.
    // neighbour's north = child's north; child's north = child's north + neighbour's height + gap
    fn swap_with_south_neighbour(footprints: &mut BTreeMap<ElementPath, Rect>, child: &ElementPath) {
        let Some(&footprint) = footprints.get(child) else {
            return;
        };
        let band = Self::band_of(footprints, child, footprint);
        let south_neighbours = band
            .into_iter()
            .filter(|(_path, sibling)| sibling.y0 >= footprint.y1);
        let nearest_south_neighbour = south_neighbours.min_by(|(path, sibling), (other_path, other_sibling)| {
            sibling
                .y0
                .total_cmp(&other_sibling.y0)
                .then_with(|| path.cmp(other_path))
        });
        let Some((neighbour, neighbour_footprint)) = nearest_south_neighbour else {
            return;
        };
        let gap = neighbour_footprint.y0 - footprint.y1;
        let neighbour_height = neighbour_footprint.height();
        let child_old_north = footprint.y0;
        let child_north = child_old_north + neighbour_height + gap;
        Self::move_north_edge_to(footprints, &neighbour, neighbour_footprint, child_old_north);
        Self::move_north_edge_to(footprints, child, footprint, child_north);
    }

    /// Return the siblings of `child`, footprinted `footprint`, sharing part of its plan x-range: its rank band.
    fn band_of(
        footprints: &BTreeMap<ElementPath, Rect>,
        child: &ElementPath,
        footprint: Rect,
    ) -> Vec<(ElementPath, Rect)> {
        footprints
            .iter()
            .filter(|&(path, sibling)| {
                let is_sibling = path != child;
                let is_sharing_x = sibling.x0 < footprint.x1 && sibling.x1 > footprint.x0;
                is_sibling && is_sharing_x
            })
            .map(|(path, sibling)| (path.clone(), *sibling))
            .collect()
    }

    /// Move `path`'s `footprint` along plan y so its north edge lies at `north`.
    fn move_north_edge_to(
        footprints: &mut BTreeMap<ElementPath, Rect>,
        path: &ElementPath,
        footprint: Rect,
        north: f64,
    ) {
        let shift = north - footprint.y0;
        let shift_offset = Vec2::new(0.0, shift);
        let moved_footprint = footprint + shift_offset;
        let path_key = path.clone();
        footprints.insert(path_key, moved_footprint);
    }
}
