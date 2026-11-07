//! Format Bézier paths as SVG path data with fixed-decimal coordinates.

use std::fmt;

use kurbo::{BezPath, PathEl, Point};

use super::FixedDecimal;

/// Formats a path as absolute `d` commands, every coordinate through FixedDecimal.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PathData<'p>(pub(crate) &'p BezPath);

impl fmt::Display for PathData<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let PathData(path) = *self;
        for (element_index, element) in path.elements().iter().enumerate() {
            if element_index > 0 {
                formatter.write_str(" ")?;
            }
            match *element {
                PathEl::MoveTo(point) => Self::write_command(formatter, 'M', &[point])?,
                PathEl::LineTo(point) => Self::write_command(formatter, 'L', &[point])?,
                PathEl::QuadTo(control, point) => {
                    Self::write_command(formatter, 'Q', &[control, point])?;
                }
                PathEl::CurveTo(first_control, second_control, point) => {
                    let points = [first_control, second_control, point];
                    Self::write_command(formatter, 'C', &points)?;
                }
                PathEl::ClosePath => formatter.write_str("Z")?,
            }
        }
        Ok(())
    }
}

impl PathData<'_> {
    /// Write `command` then its `points`, each as `x,y` in fixed decimals.
    fn write_command(
        formatter: &mut fmt::Formatter<'_>,
        command: char,
        points: &[Point],
    ) -> fmt::Result {
        write!(formatter, "{command}")?;
        for (point_index, point) in points.iter().enumerate() {
            if point_index > 0 {
                formatter.write_str(" ")?;
            }
            let x = FixedDecimal(point.x);
            let y = FixedDecimal(point.y);
            write!(formatter, "{x},{y}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_is_written_absolute_with_fixed_decimals() {
        let mut path = BezPath::new();
        path.move_to((0.0, -0.0));
        path.line_to((1.000_4, 2.5));
        path.quad_to((3.0, 4.0), (5.123_456, -6.0));
        path.curve_to((7.0, 8.0), (9.0, 10.0), (11.0, 12.999_9));
        path.close_path();
        let path_data = PathData(&path).to_string();
        assert_eq!(path_data, "M0,0 L1,2.5 Q3,4 5.123,-6 C7,8 9,10 11,13 Z");
    }

    #[test]
    fn empty_path_has_empty_data() {
        let path = BezPath::new();
        assert_eq!(PathData(&path).to_string(), "");
    }
}
