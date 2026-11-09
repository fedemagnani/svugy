//! Rearrange a level's resized children from the order the deepest layer placed them in: keep every pair's order along each axis, shorten the edges between them, and let each child follow its nearest predecessor, so every layer shows the arrangement its reader already saw, compacted.

use std::collections::{BTreeMap, BTreeSet};

use kurbo::{Rect, Size, Vec2};
use svugy_model::path::ElementPath;

use super::LEVEL_MARGIN;

/// Gap kept at most between two children facing each other, the compound layout's gap within a rank.
const FACING_GAP: f64 = 48.0;
/// Weight of keeping a child's seed gap to its nearest facing predecessor, against one squared unit of edge length.
const SEED_GAP_WEIGHT: f64 = 0.05;
/// Weight of staying at the seed position, only enough to make the optimum unique.
const SEED_POSITION_WEIGHT: f64 = 1e-5;
/// Dual sweeps a solve may run before taking its multipliers as converged.
const MAX_SWEEPS: usize = 10_000;
/// Largest multiplier change a sweep may make for the solve to count as converged.
const CONVERGENCE: f64 = 1e-12;

/// Holds a level's footprints as the deepest layer placed them, to resize its children without reordering them.
pub(super) struct SeedArrangement<'s> {
    seed: &'s BTreeMap<ElementPath, Rect>,
}

impl<'s> SeedArrangement<'s> {
    /// Arrange from the deepest layer's `seed` footprints of a ranked level, which never holds a pinned child.
    pub(super) fn new(seed: &'s BTreeMap<ElementPath, Rect>) -> Self {
        Self { seed }
    }

    /// Resize every child to `sizes` and place it along plan x, then plan y, by the program of each axis: the seed itself when no size changed.
    pub(super) fn rearranged(
        &self,
        sizes: &BTreeMap<ElementPath, Size>,
        joined_pairs: &BTreeSet<(ElementPath, ElementPath)>,
    ) -> BTreeMap<ElementPath, Rect> {
        let is_seed_sized = self.seed.iter().all(|(path, seed_footprint)| {
            let seed_size = seed_footprint.size();
            sizes[path] == seed_size
        });
        if is_seed_sized {
            return self.seed.clone();
        }
        let resized = self.resized_at_seed_corners(sizes);
        let placed_along_x = self.placed_along::<Horizontal>(&resized, joined_pairs);
        self.placed_along::<Vertical>(&placed_along_x, joined_pairs)
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

    /// Place every footprint along `A` at the optimum of the axis program built from the seed.
    fn placed_along<A: Axis>(
        &self,
        footprints: &BTreeMap<ElementPath, Rect>,
        joined_pairs: &BTreeSet<(ElementPath, ElementPath)>,
    ) -> BTreeMap<ElementPath, Rect> {
        let program = AxisProgram::of::<A>(self.seed, footprints, joined_pairs);
        let starts = program.solved_starts();
        footprints
            .iter()
            .zip(starts)
            .map(|((path, footprint), start)| {
                let current_start = A::start(footprint);
                let shift = start - current_start;
                let placed_footprint = A::shifted(*footprint, shift);
                (path.clone(), placed_footprint)
            })
            .collect()
    }
}

/// Places children along one axis by a convex quadratic program over their starts x: minimize ½xᵀQx + cᵀx subject to every separation, Q positive definite.
struct AxisProgram {
    hessian: Vec<Vec<f64>>,
    linear: Vec<f64>,
    separations: Vec<Separation>,
    /// Child indices in order of their seed starts, the order every separation runs forward in.
    seed_order: Vec<usize>,
}

/// Keeps child `after` at least `min_distance` past child `before` along the axis: x_after − x_before ≥ min_distance.
struct Separation {
    before: usize,
    after: usize,
    min_distance: f64,
}

impl AxisProgram {
    /// Build the program along `A` for `footprints`, ordered as the seed orders them: separations keep every pair apart in the seed apart and ordered, and the objective sums the squared center distance of every joined pair, the weighted squared change of each child's seed gap to its nearest facing predecessor, and a faint pull to the seed.
    // f(x) = Σ_(u,v) (c_u − c_v)² + λ Σ_j (x_j − x_p(j) − s_p(j) − g_j)² + ε Σ_i (x_i − x⁰_i)²
    fn of<A: Axis>(
        seed: &BTreeMap<ElementPath, Rect>,
        footprints: &BTreeMap<ElementPath, Rect>,
        joined_pairs: &BTreeSet<(ElementPath, ElementPath)>,
    ) -> Self {
        let paths: Vec<&ElementPath> = footprints.keys().collect();
        let index_by_path: BTreeMap<&ElementPath, usize> = paths
            .iter()
            .enumerate()
            .map(|(index, path)| (*path, index))
            .collect();
        let seeds: Vec<Rect> = paths.iter().map(|path| seed[*path]).collect();
        let sizes: Vec<f64> = footprints
            .values()
            .map(|footprint| A::end(footprint) - A::start(footprint))
            .collect();
        let child_count = paths.len();
        let mut program = Self {
            hessian: vec![vec![0.0; child_count]; child_count],
            linear: vec![0.0; child_count],
            separations: Vec::new(),
            seed_order: Self::seed_order_of::<A>(&seeds),
        };
        for (index, seed_footprint) in seeds.iter().enumerate() {
            let seed_start = A::start(seed_footprint);
            program.add_anchor(index, seed_start, SEED_POSITION_WEIGHT);
        }
        for (after, after_seed) in seeds.iter().enumerate() {
            let mut nearest_facing: Option<(usize, f64)> = None;
            for (before, before_seed) in seeds.iter().enumerate() {
                let is_apart_before = A::is_apart_before(before_seed, after_seed);
                if !is_apart_before {
                    continue;
                }
                let seed_gap = A::start(after_seed) - A::end(before_seed);
                let is_facing = A::is_facing(before_seed, after_seed);
                let kept_gap = if is_facing {
                    seed_gap.min(FACING_GAP)
                } else {
                    seed_gap.min(LEVEL_MARGIN)
                };
                let min_distance = sizes[before] + kept_gap;
                let separation = Separation {
                    before,
                    after,
                    min_distance,
                };
                program.separations.push(separation);
                let before_end = A::end(before_seed);
                let is_nearer = nearest_facing.is_none_or(|(nearest, _gap)| {
                    let nearest_end = A::end(&seeds[nearest]);
                    before_end > nearest_end
                });
                if is_facing && is_nearer {
                    nearest_facing = Some((before, seed_gap));
                }
            }
            let Some((nearest, seed_gap)) = nearest_facing else {
                continue;
            };
            let kept_distance = sizes[nearest] + seed_gap;
            program.add_difference(after, nearest, kept_distance, SEED_GAP_WEIGHT);
        }
        for (from, to) in joined_pairs {
            let (Some(&from_index), Some(&to_index)) =
                (index_by_path.get(from), index_by_path.get(to))
            else {
                continue;
            };
            let is_self_pair = from_index == to_index;
            if is_self_pair {
                continue;
            }
            let center_offset = (sizes[to_index] - sizes[from_index]) / 2.0;
            program.add_difference(from_index, to_index, center_offset, 1.0);
        }
        program
    }

    /// Return the children's starts at the program's optimum, any tolerance-sized violation of a separation pushed forward in seed order.
    fn solved_starts(&self) -> Vec<f64> {
        let mut starts = self.optimum_by_hildreth();
        for &after in &self.seed_order {
            let incoming = self
                .separations
                .iter()
                .filter(|separation| separation.after == after);
            for separation in incoming {
                let least_start = starts[separation.before] + separation.min_distance;
                let start = starts[after];
                starts[after] = start.max(least_start);
            }
        }
        starts
    }

    /// Solve the program by Hildreth's dual coordinate descent on the separations' multipliers μ ≥ 0.
    // x(μ) = x₀ + Q⁻¹Aᵀμ, x₀ = −Q⁻¹c; minimize ½μᵀHμ − kᵀμ with H = AQ⁻¹Aᵀ, k = b − Ax₀; μ_k ← max(0, μ_k − ((Hμ)_k − k_k)/H_kk)
    fn optimum_by_hildreth(&self) -> Vec<f64> {
        let inverse_hessian = invert(&self.hessian);
        let child_count = self.linear.len();
        let unconstrained: Vec<f64> = (0..child_count)
            .map(|row| {
                let products = inverse_hessian[row]
                    .iter()
                    .zip(&self.linear)
                    .map(|(entry, linear)| entry * linear);
                -products.sum::<f64>()
            })
            .collect();
        let dual_hessian: Vec<Vec<f64>> = self
            .separations
            .iter()
            .map(|row_separation| {
                self.separations
                    .iter()
                    .map(|column_separation| {
                        Self::dual_entry(&inverse_hessian, row_separation, column_separation)
                    })
                    .collect()
            })
            .collect();
        let slacks: Vec<f64> = self
            .separations
            .iter()
            .map(|separation| {
                let unconstrained_distance =
                    unconstrained[separation.after] - unconstrained[separation.before];
                separation.min_distance - unconstrained_distance
            })
            .collect();
        let mut multipliers = vec![0.0; self.separations.len()];
        for _sweep in 0..MAX_SWEEPS {
            let mut largest_change: f64 = 0.0;
            for (index, dual_row) in dual_hessian.iter().enumerate() {
                let pairs = dual_row.iter().zip(&multipliers);
                let row_product: f64 = pairs.map(|(entry, multiplier)| entry * multiplier).sum();
                let gradient = row_product - slacks[index];
                let diagonal = dual_row[index];
                let stepped = multipliers[index] - gradient / diagonal;
                let clamped = stepped.max(0.0);
                let change = (clamped - multipliers[index]).abs();
                largest_change = largest_change.max(change);
                multipliers[index] = clamped;
            }
            let is_converged = largest_change < CONVERGENCE;
            if is_converged {
                break;
            }
        }
        let mut pushes = vec![0.0; child_count];
        for (separation, multiplier) in self.separations.iter().zip(&multipliers) {
            pushes[separation.after] += multiplier;
            pushes[separation.before] -= multiplier;
        }
        (0..child_count)
            .map(|row| {
                let products = inverse_hessian[row]
                    .iter()
                    .zip(&pushes)
                    .map(|(entry, push)| entry * push);
                unconstrained[row] + products.sum::<f64>()
            })
            .collect()
    }

    /// Return the entry of AQ⁻¹Aᵀ for two separations, each row of A being +1 at its after and −1 at its before.
    // H_kl = Q⁻¹[a_k][a_l] − Q⁻¹[a_k][b_l] − Q⁻¹[b_k][a_l] + Q⁻¹[b_k][b_l]
    fn dual_entry(inverse_hessian: &[Vec<f64>], row: &Separation, column: &Separation) -> f64 {
        let after_after = inverse_hessian[row.after][column.after];
        let after_before = inverse_hessian[row.after][column.before];
        let before_after = inverse_hessian[row.before][column.after];
        let before_before = inverse_hessian[row.before][column.before];
        after_after - after_before - before_after + before_before
    }

    /// Add weight·(x_index − target)² to the objective.
    fn add_anchor(&mut self, index: usize, target: f64, weight: f64) {
        self.hessian[index][index] += 2.0 * weight;
        self.linear[index] -= 2.0 * weight * target;
    }

    /// Add weight·(x_first − x_second − target)² to the objective.
    fn add_difference(&mut self, first: usize, second: usize, target: f64, weight: f64) {
        let doubled_weight = 2.0 * weight;
        self.hessian[first][first] += doubled_weight;
        self.hessian[second][second] += doubled_weight;
        self.hessian[first][second] -= doubled_weight;
        self.hessian[second][first] -= doubled_weight;
        self.linear[first] -= doubled_weight * target;
        self.linear[second] += doubled_weight * target;
    }

    /// Return child indices in order of their seed starts along `A`, index order breaking ties.
    fn seed_order_of<A: Axis>(seeds: &[Rect]) -> Vec<usize> {
        let mut seed_order: Vec<usize> = (0..seeds.len()).collect();
        seed_order.sort_by(|&index, &other_index| {
            let start = A::start(&seeds[index]);
            let other_start = A::start(&seeds[other_index]);
            start.total_cmp(&other_start).then(index.cmp(&other_index))
        });
        seed_order
    }
}

/// Invert the positive definite `matrix` by Gauss–Jordan elimination with partial pivoting.
fn invert(matrix: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let size = matrix.len();
    let mut left: Vec<Vec<f64>> = matrix.to_vec();
    let mut right: Vec<Vec<f64>> = (0..size)
        .map(|row| {
            (0..size)
                .map(|column| f64::from(u8::from(row == column)))
                .collect()
        })
        .collect();
    for pivot in 0..size {
        let pivot_row = (pivot..size)
            .max_by(|&row, &other_row| {
                let magnitude = left[row][pivot].abs();
                let other_magnitude = left[other_row][pivot].abs();
                magnitude
                    .total_cmp(&other_magnitude)
                    .then(other_row.cmp(&row))
            })
            .expect("a pivot column holds at least its own row");
        left.swap(pivot, pivot_row);
        right.swap(pivot, pivot_row);
        let pivot_value = left[pivot][pivot];
        for column in 0..size {
            left[pivot][column] /= pivot_value;
            right[pivot][column] /= pivot_value;
        }
        for row in 0..size {
            let is_pivot_row = row == pivot;
            if is_pivot_row {
                continue;
            }
            let factor = left[row][pivot];
            for column in 0..size {
                let left_step = factor * left[pivot][column];
                let right_step = factor * right[pivot][column];
                left[row][column] -= left_step;
                right[row][column] -= right_step;
            }
        }
    }
    right
}

/// Read and shift footprints along one plan axis, so the x and y programs share one builder.
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
    fn a_closing_container_pulls_its_neighbours_in_and_keeps_every_order() {
        let first = ElementPath::from_segments(["first"]);
        let container = ElementPath::from_segments(["container"]);
        let connected = ElementPath::from_segments(["connected"]);
        let beside = ElementPath::from_segments(["beside"]);
        let seed = BTreeMap::from([
            (first.clone(), Rect::new(0.0, 0.0, 100.0, 50.0)),
            (container.clone(), Rect::new(148.0, 0.0, 648.0, 300.0)),
            (connected.clone(), Rect::new(700.0, 100.0, 800.0, 150.0)),
            (beside.clone(), Rect::new(500.0, 350.0, 600.0, 400.0)),
        ]);
        let joined_pairs = BTreeSet::from([(container.clone(), connected.clone())]);
        let arrangement = SeedArrangement::new(&seed);

        let seed_sizes: BTreeMap<ElementPath, Size> = seed
            .iter()
            .map(|(path, seed_footprint)| (path.clone(), seed_footprint.size()))
            .collect();
        let unchanged = arrangement.rearranged(&seed_sizes, &joined_pairs);
        assert_eq!(
            unchanged, seed,
            "a level whose children keep their seed sizes must keep the seed exactly"
        );

        let mut closed_sizes = seed_sizes.clone();
        closed_sizes.insert(container.clone(), Size::new(100.0, 60.0));
        let rearranged = arrangement.rearranged(&closed_sizes, &joined_pairs);

        let closed_container = rearranged[&container];
        let pulled_connected = rearranged[&connected];
        let connected_gap = pulled_connected.x0 - closed_container.x1;
        assert!(
            connected_gap <= FACING_GAP + 1e-6,
            "a child joined to a closing container must follow it in, keeping at most the facing gap: {connected_gap}"
        );
        let pulled_beside = rearranged[&beside];
        assert!(
            pulled_beside.y0 >= closed_container.y1,
            "a child south of the container in the seed must stay south of it"
        );
        let seed_beside = seed[&beside];
        assert!(
            pulled_beside.y0 < seed_beside.y0,
            "a child south of a container shrinking along y must move north with it"
        );
        assert_no_two_footprints_overlap(&rearranged);
    }

    fn assert_no_two_footprints_overlap(footprints: &BTreeMap<ElementPath, Rect>) {
        for (left_path, left) in footprints {
            for (right_path, right) in footprints.range(left_path..).skip(1) {
                let overlap = left.intersect(*right);
                let overlap_area = overlap.area();
                assert!(
                    overlap_area <= 1e-9,
                    "no two footprints may overlap: {left_path:?} and {right_path:?} do"
                );
            }
        }
    }
}
