//! See the diagram from above a ground corner.

use glam::DVec3;
use svugy_model::model::ViewAngle;

use super::Camera;

/// Looks down along (−1, −1, −1): both ground axes recede at 30° from the screen's horizontal.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Isometric;

impl Camera for Isometric {
    const ANGLE: ViewAngle = ViewAngle::Isometric;
    /// screen x = (x − y)·cos 30°.
    const SCREEN_X_ROW: DVec3 = DVec3::new(Self::COS_30, -Self::COS_30, 0.0);
    /// screen y = (x + y)·sin 30° − z.
    const SCREEN_Y_ROW: DVec3 = DVec3::new(Self::SIN_30, Self::SIN_30, -1.0);
    const TOWARD_VIEWER: DVec3 = DVec3::ONE;
}

impl Isometric {
    /// √3/2, the cosine of the 30° each ground axis makes with the screen's horizontal.
    const COS_30: f64 = 0.866_025_403_784_438_6;
    /// 1/2, the sine of the 30° each ground axis makes with the screen's horizontal.
    const SIN_30: f64 = 0.5;
}

#[cfg(test)]
mod tests {
    use kurbo::Point;

    use super::*;

    #[test]
    fn unit_axes_land_where_the_example_draws_them() {
        let origin = Isometric::project(DVec3::ZERO);
        let x_axis = Isometric::project(DVec3::X);
        let y_axis = Isometric::project(DVec3::Y);
        let z_axis = Isometric::project(DVec3::Z);
        assert_eq!(origin, Point::ORIGIN);
        assert_eq!(x_axis, Point::new(0.866_025_403_784_438_6, 0.5));
        assert_eq!(y_axis, Point::new(-0.866_025_403_784_438_6, 0.5));
        assert_eq!(z_axis, Point::new(0.0, -1.0));
    }

    #[test]
    fn known_plan_points_project_by_the_formula() {
        let plan_point = DVec3::new(140.0, 50.0, 22.5);
        let screen_point = Isometric::project(plan_point);
        let expected_x = 90.0 * 0.866_025_403_784_438_6;
        let expected_y = 95.0 - 22.5;
        assert!((screen_point.x - expected_x).abs() < 1e-12);
        assert_eq!(screen_point.y, expected_y);
    }

    #[test]
    fn top_face_plane_matches_the_example_label_matrix() {
        let origin = DVec3::new(10.0, 20.0, 5.0);
        let plane = Isometric::plane(origin, DVec3::X, DVec3::Y);
        let [a, b, c, d, e, f] = plane.as_coeffs();
        let projected_origin = Isometric::project(origin);
        assert_eq!(
            [a, b, c, d],
            [0.866_025_403_784_438_6, 0.5, -0.866_025_403_784_438_6, 0.5]
        );
        assert_eq!(Point::new(e, f), projected_origin);
    }

    #[test]
    fn horizontal_plane_agrees_with_point_projection() {
        let plane = Isometric::horizontal_plane_at(7.0);
        let plan_point = Point::new(33.0, -12.0);
        let through_plane = plane * plan_point;
        let through_projection = Isometric::project(DVec3::new(33.0, -12.0, 7.0));
        assert!((through_plane - through_projection).hypot2().sqrt() < 1e-12);
    }
}
