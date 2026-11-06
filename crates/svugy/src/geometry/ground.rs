//! Rule the ground grid under a level.

use std::iter;
use std::ops::RangeInclusive;

use kurbo::{Line, Point, Rect};

use super::projection::IsometricProjection;

/// Rules the ground with lines of constant plan x and of constant plan y, one cell apart and one cell past the level's boundary.
pub(super) struct GroundGrid {
    cell: f64,
    columns: RangeInclusive<i64>,
    rows: RangeInclusive<i64>,
}

impl GroundGrid {
    /// Spacing of the example's grid, in plan units.
    const EXAMPLE_CELL: f64 = 60.0;
    /// Most lines ruled along either axis, so the grid stays small however far the boundary reaches.
    const MAX_LINES_PER_AXIS: i64 = 128;

    /// Cover `boundary` with whole cells, plus one more on every side, in the smallest cell that keeps both axes within the line cap.
    pub(super) fn covering(boundary: Rect) -> Self {
        let cells_doubling_from_the_example =
            iter::successors(Some(Self::EXAMPLE_CELL), |cell| Some(cell * 2.0));
        let mut grids =
            cells_doubling_from_the_example.map(|cell| Self::covering_in_cells(boundary, cell));
        grids
            .find(Self::is_within_line_cap)
            .expect("doubling cells eventually cover any finite boundary within the cap")
    }

    /// Return the projected lines of constant x, west to east, then those of constant y, north to south.
    pub(super) fn lines(&self) -> Vec<Line> {
        let ground = IsometricProjection::horizontal_plane_at(0.0);
        let north_y = self.cell_coordinate(*self.rows.start());
        let south_y = self.cell_coordinate(*self.rows.end());
        let west_x = self.cell_coordinate(*self.columns.start());
        let east_x = self.cell_coordinate(*self.columns.end());
        let constant_x_lines = self.columns.clone().map(|column| {
            let x = self.cell_coordinate(column);
            let north_end = Point::new(x, north_y);
            let south_end = Point::new(x, south_y);
            ground * Line::new(north_end, south_end)
        });
        let constant_y_lines = self.rows.clone().map(|row| {
            let y = self.cell_coordinate(row);
            let west_end = Point::new(west_x, y);
            let east_end = Point::new(east_x, y);
            ground * Line::new(west_end, east_end)
        });
        constant_x_lines.chain(constant_y_lines).collect()
    }

    fn covering_in_cells(boundary: Rect, cell: f64) -> Self {
        let first_column = Self::cell_index_at_or_below(boundary.x0, cell) - 1;
        let last_column = Self::cell_index_at_or_above(boundary.x1, cell) + 1;
        let first_row = Self::cell_index_at_or_below(boundary.y0, cell) - 1;
        let last_row = Self::cell_index_at_or_above(boundary.y1, cell) + 1;
        Self {
            cell,
            columns: first_column..=last_column,
            rows: first_row..=last_row,
        }
    }

    fn is_within_line_cap(&self) -> bool {
        let column_line_count = self.columns.end() - self.columns.start() + 1;
        let row_line_count = self.rows.end() - self.rows.start() + 1;
        let is_column_count_within_cap = column_line_count <= Self::MAX_LINES_PER_AXIS;
        let is_row_count_within_cap = row_line_count <= Self::MAX_LINES_PER_AXIS;
        is_column_count_within_cap && is_row_count_within_cap
    }

    fn cell_index_at_or_below(coordinate: f64, cell: f64) -> i64 {
        let cells = coordinate / cell;
        cells.floor() as i64
    }

    fn cell_index_at_or_above(coordinate: f64, cell: f64) -> i64 {
        let cells = coordinate / cell;
        cells.ceil() as i64
    }

    fn cell_coordinate(&self, index: i64) -> f64 {
        index as f64 * self.cell
    }
}

#[cfg(test)]
mod tests {
    use glam::DVec3;

    use super::*;

    #[test]
    fn grid_reaches_one_cell_past_the_boundary_at_the_example_spacing() {
        let boundary = Rect::new(0.0, 0.0, 220.0, 110.0);
        let grid = GroundGrid::covering(boundary);
        assert_eq!(grid.columns, -1..=5);
        assert_eq!(grid.rows, -1..=3);
        let lines = grid.lines();
        assert_eq!(lines.len(), 7 + 5);
        let first_line = lines[0];
        let north_west = IsometricProjection::project(DVec3::new(-60.0, -60.0, 0.0));
        let south_west = IsometricProjection::project(DVec3::new(-60.0, 180.0, 0.0));
        assert!((first_line.p0 - north_west).hypot2().sqrt() < 1e-9);
        assert!((first_line.p1 - south_west).hypot2().sqrt() < 1e-9);
    }

    #[test]
    fn neighbouring_lines_are_one_cell_apart_like_the_example() {
        let boundary = Rect::new(0.0, 0.0, 120.0, 120.0);
        let lines = GroundGrid::covering(boundary).lines();
        let step = lines[1].p0 - lines[0].p0;
        assert!((step.x - 52.0).abs() < 0.05);
        assert!((step.y - 30.0).abs() < 1e-9);
    }

    #[test]
    fn a_boundary_at_the_magnitude_bound_rules_a_capped_grid_that_still_covers_it() {
        let boundary = Rect::new(0.0, 0.0, 2_000_000.0, 60.0);
        let grid = GroundGrid::covering(boundary);
        let lines = grid.lines();
        let max_line_count = 2 * GroundGrid::MAX_LINES_PER_AXIS as usize;
        assert!(lines.len() <= max_line_count, "{} lines", lines.len());
        let west_x = grid.cell_coordinate(*grid.columns.start());
        let east_x = grid.cell_coordinate(*grid.columns.end());
        assert!(west_x < boundary.x0 && east_x > boundary.x1);
        assert_eq!(grid.rows, -1..=2);
    }
}
