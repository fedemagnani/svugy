//! Project plan space onto the isometric screen.

use glam::{DMat3, DVec3};
use kurbo::{Affine, Point};

/// Projects plan points (x and y on the ground, z up) onto the screen (x right, y down).
pub(super) struct IsometricProjection;

impl IsometricProjection {
    /// √3/2, the cosine of the 30° each ground axis makes with the screen's horizontal.
    const COS_30: f64 = 0.866_025_403_784_438_6;
    const SIN_30: f64 = 0.5;

    /// screen = ((x − y)·cos 30°, (x + y)·sin 30° − z); the third row is always zero.
    const MATRIX: DMat3 = DMat3::from_cols(
        DVec3::new(Self::COS_30, Self::SIN_30, 0.0),
        DVec3::new(-Self::COS_30, Self::SIN_30, 0.0),
        DVec3::new(0.0, -1.0, 0.0),
    );

    pub(super) fn project(plan_point: DVec3) -> Point {
        let screen_point = Self::MATRIX * plan_point;
        Point::new(screen_point.x, screen_point.y)
    }

    /// Map plane coordinates (s, t) to the screen point P(origin + s·u + t·v), for the plane through `origin` spanned by `u` and `v`.
    pub(super) fn plane(origin: DVec3, u: DVec3, v: DVec3) -> Affine {
        let projected_u = Self::MATRIX * u;
        let projected_v = Self::MATRIX * v;
        let projected_origin = Self::MATRIX * origin;
        Affine::new([
            projected_u.x,
            projected_u.y,
            projected_v.x,
            projected_v.y,
            projected_origin.x,
            projected_origin.y,
        ])
    }

    /// Map plan (x, y) on the horizontal plane at height `z` to the screen.
    pub(super) fn horizontal_plane_at(z: f64) -> Affine {
        let origin = DVec3::new(0.0, 0.0, z);
        Self::plane(origin, DVec3::X, DVec3::Y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_axes_land_where_the_example_draws_them() {
        let origin = IsometricProjection::project(DVec3::ZERO);
        let x_axis = IsometricProjection::project(DVec3::X);
        let y_axis = IsometricProjection::project(DVec3::Y);
        let z_axis = IsometricProjection::project(DVec3::Z);
        assert_eq!(origin, Point::ORIGIN);
        assert_eq!(x_axis, Point::new(0.866_025_403_784_438_6, 0.5));
        assert_eq!(y_axis, Point::new(-0.866_025_403_784_438_6, 0.5));
        assert_eq!(z_axis, Point::new(0.0, -1.0));
    }

    #[test]
    fn known_plan_points_project_by_the_formula() {
        let plan_point = DVec3::new(140.0, 50.0, 22.5);
        let screen_point = IsometricProjection::project(plan_point);
        let expected_x = 90.0 * 0.866_025_403_784_438_6;
        let expected_y = 95.0 - 22.5;
        assert!((screen_point.x - expected_x).abs() < 1e-12);
        assert_eq!(screen_point.y, expected_y);
    }

    #[test]
    fn top_face_plane_matches_the_example_label_matrix() {
        let origin = DVec3::new(10.0, 20.0, 5.0);
        let plane = IsometricProjection::plane(origin, DVec3::X, DVec3::Y);
        let [a, b, c, d, e, f] = plane.as_coeffs();
        let projected_origin = IsometricProjection::project(origin);
        assert_eq!(
            [a, b, c, d],
            [0.866_025_403_784_438_6, 0.5, -0.866_025_403_784_438_6, 0.5]
        );
        assert_eq!(Point::new(e, f), projected_origin);
    }

    #[test]
    fn horizontal_plane_agrees_with_point_projection() {
        let plane = IsometricProjection::horizontal_plane_at(7.0);
        let plan_point = Point::new(33.0, -12.0);
        let through_plane = plane * plan_point;
        let through_projection = IsometricProjection::project(DVec3::new(33.0, -12.0, 7.0));
        assert!((through_plane - through_projection).hypot2().sqrt() < 1e-12);
    }
}
