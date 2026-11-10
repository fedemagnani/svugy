//! Project plan space onto the screen from each angle a diagram is seen from.

use glam::DVec3;
use kurbo::{Affine, Point};
use svugy_model::model::ViewAngle;

pub(crate) use self::isometric::Isometric;
pub(crate) use self::top::Top;

mod isometric;
mod top;

/// Projects plan points (x and y on the ground, z up) onto the screen (x right, y down) from one fixed angle.
pub(crate) trait Camera {
    /// Angle this camera sees the diagram from.
    const ANGLE: ViewAngle;
    /// Plan weights of screen x: screen x = SCREEN_X_ROW · p.
    const SCREEN_X_ROW: DVec3;
    /// Plan weights of screen y: screen y = SCREEN_Y_ROW · p.
    const SCREEN_Y_ROW: DVec3;
    /// Plan step along the sight line toward the viewer per unit of height: SCREEN_X_ROW × SCREEN_Y_ROW scaled to z = 1.
    const TOWARD_VIEWER: DVec3;
    /// Plan direction a tag hanging from a solid's front face runs down the screen.
    const HANGING_DOWN: DVec3 = DVec3::NEG_Z;
    /// Size of a connection's label against the size its text is measured at.
    const CONNECTION_LABEL_SCALE: f64 = 1.0;

    /// Project `plan_point` onto the screen.
    fn project(plan_point: DVec3) -> Point {
        let screen_x = Self::SCREEN_X_ROW.dot(plan_point);
        let screen_y = Self::SCREEN_Y_ROW.dot(plan_point);
        Point::new(screen_x, screen_y)
    }

    /// Map plane coordinates (s, t) to the screen point P(origin + s·u + t·v), for the plane through `origin` spanned by `u` and `v`.
    fn plane(origin: DVec3, u: DVec3, v: DVec3) -> Affine {
        let projected_u = Self::project(u);
        let projected_v = Self::project(v);
        let projected_origin = Self::project(origin);
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
    fn horizontal_plane_at(z: f64) -> Affine {
        let origin = DVec3::new(0.0, 0.0, z);
        Self::plane(origin, DVec3::X, DVec3::Y)
    }

    /// Tell whether the viewer sees a flat face whose outward normal is `outward_normal`: n · TOWARD_VIEWER > 0.
    fn sees(outward_normal: DVec3) -> bool {
        let facing = outward_normal.dot(Self::TOWARD_VIEWER);
        facing > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_sight_line_projects_to_a_point<C: Camera>() {
        let sight_point = C::project(C::TOWARD_VIEWER);
        let rise = C::TOWARD_VIEWER.z;
        assert!(
            sight_point.to_vec2().hypot2() < 1e-24 && rise == 1.0,
            "{:?}: TOWARD_VIEWER must rise one unit along the sight line, which projects every point onto one screen point",
            C::ANGLE
        );
    }

    #[test]
    fn every_cameras_sight_line_rises_one_unit_and_projects_to_a_point() {
        assert_sight_line_projects_to_a_point::<Isometric>();
        assert_sight_line_projects_to_a_point::<Top>();
    }
}
