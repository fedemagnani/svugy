//! Reuse ranked placements across the layouts of one model: a level's compound placement depends only on the model and on the extents of the levels below it, so an arrangement search recomputes it only when those change.

use std::cell::RefCell;
use std::collections::BTreeMap;

use svugy_model::model::Model;
use svugy_model::path::ElementPath;

use super::RankedPlacement;
use crate::layout::edge_levels::LeavingEdges;
use crate::layout::{PlacedLevel, PlacedLevels};
use crate::metrics::TextMeasure;

/// Names one ranked placement by its inputs beside the model: the level, and the exact extent of every level placed below it.
type PlacementKey = (ElementPath, Vec<(ElementPath, u64, u64)>);

/// Holds the ranked placements already computed for one model, keyed by their inputs, for every layout of that model to reuse.
#[derive(Default)]
pub(crate) struct PlacementCache {
    placements: RefCell<BTreeMap<PlacementKey, Vec<(ElementPath, PlacedLevel)>>>,
}

/// Places ranked levels with `inner`, returning a placement `cache` already holds for the same inputs instead of computing it again.
pub(in crate::layout) struct MemoizedPlacement<'p, P> {
    inner: &'p P,
    cache: &'p PlacementCache,
}

impl PlacementCache {
    /// Hold no placement yet.
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

impl<'p, P> MemoizedPlacement<'p, P> {
    /// Place with `inner`, remembering its placements in `cache`.
    pub(in crate::layout) fn new(inner: &'p P, cache: &'p PlacementCache) -> Self {
        Self { inner, cache }
    }
}

impl<P: RankedPlacement> RankedPlacement for MemoizedPlacement<'_, P> {
    /// Return the cached placement of `level` for the extents placed below it, or place it with the inner placement and cache it.
    fn place_ranked_level(
        &self,
        model: &Model,
        level: &ElementPath,
        leaving_edges: &LeavingEdges,
        interiors: &PlacedLevels,
        measure: &impl TextMeasure,
    ) -> Vec<(ElementPath, PlacedLevel)> {
        let key = Self::key_of(level, interiors);
        let cached = {
            let placements = self.cache.placements.borrow();
            let cached_placement = placements.get(&key);
            cached_placement.cloned()
        };
        if let Some(cached_placement) = cached {
            return cached_placement;
        }
        let placement = self
            .inner
            .place_ranked_level(model, level, leaving_edges, interiors, measure);
        let cached_placement = placement.clone();
        let mut placements = self.cache.placements.borrow_mut();
        placements.insert(key, cached_placement);
        placement
    }
}

impl<P> MemoizedPlacement<'_, P> {
    /// Key `level`'s placement by the exact extents, as bit patterns, of every level placed below it.
    fn key_of(level: &ElementPath, interiors: &PlacedLevels) -> PlacementKey {
        let descendant_extents = interiors
            .by_level
            .iter()
            .filter(|(placed_level, _placed)| level.is_ancestor_of(placed_level))
            .map(|(placed_level, placed)| {
                let width_bits = placed.extent.width.to_bits();
                let height_bits = placed.extent.height.to_bits();
                (placed_level.clone(), width_bits, height_bits)
            })
            .collect();
        let level_key = level.clone();
        (level_key, descendant_extents)
    }
}
