//! See the diagram from straight above, as a flat plan.

use glam::DVec3;
use svugy_model::model::ViewAngle;

use super::Camera;

/// Looks straight down: the screen is the plan, every height and every side face collapses.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Top;

impl Camera for Top {
    const ANGLE: ViewAngle = ViewAngle::Top;
    const SCREEN_X_ROW: DVec3 = DVec3::X;
    const SCREEN_Y_ROW: DVec3 = DVec3::Y;
    const TOWARD_VIEWER: DVec3 = DVec3::Z;
    /// Front faces collapse onto their top edge, so a hanging tag lies flat on the ground in front of it.
    const HANGING_DOWN: DVec3 = DVec3::Y;
    /// Shrinks connection labels: with no lift, a route runs flat and short between neighbours, its label overlapping their faces at full size.
    const CONNECTION_LABEL_SCALE: f64 = 0.75;
}
