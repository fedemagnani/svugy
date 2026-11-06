//! Lay out every hierarchy level in plan space.

mod nest;
mod place;
mod route;
mod stub;

use std::collections::{BTreeMap, BTreeSet};

use kurbo::{Point, Rect, Size};
use svugy_model::model::EdgeId;
use svugy_model::path::ElementPath;

/// Holds every level's plan keyed by level path, so the root comes first and parents precede their children.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LevelPlans {
    plans_by_level: BTreeMap<ElementPath, LevelPlan>,
}

impl LevelPlans {
    pub(crate) fn new(plans_by_level: BTreeMap<ElementPath, LevelPlan>) -> Self {
        Self { plans_by_level }
    }

    /// Return the plan of the level `level` lays out: the root's for the root path, a container's own level otherwise.
    pub(crate) fn level(&self, level: &ElementPath) -> Option<&LevelPlan> {
        self.plans_by_level.get(level)
    }

    /// Return `element`'s footprint in its parent level's plan, or `None` when that level does not place it.
    pub(crate) fn footprint_of(&self, element: &ElementPath) -> Option<Rect> {
        let parent = element.parent()?;
        let parent_plan = self.plans_by_level.get(&parent)?;
        parent_plan.footprints.get(element).copied()
    }

    #[cfg(test)]
    pub(crate) fn levels(&self) -> &BTreeMap<ElementPath, LevelPlan> {
        &self.plans_by_level
    }
}

/// Plans one hierarchy level on the ground plane, in plan units, with its origin at the level's top-left corner.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LevelPlan {
    /// Size the parent level gives this container's footprint, stub margins included.
    pub(crate) extent: Size,
    /// Footprint of every child element, keyed by the child's full path.
    pub(crate) footprints: BTreeMap<ElementPath, Rect>,
    /// Route of every edge homed in this level, between its true endpoints.
    pub(crate) routes: BTreeMap<EdgeId, Route>,
    /// Stubs of the edges leaving this level, keyed by their external endpoint.
    pub(crate) stubs: BTreeMap<ElementPath, Stub>,
}

impl LevelPlan {
    pub(crate) fn new(extent: Size) -> Self {
        Self {
            extent,
            footprints: BTreeMap::new(),
            routes: BTreeMap::new(),
            stubs: BTreeMap::new(),
        }
    }

    /// Return the level's own boundary, where stub slots sit.
    pub(crate) fn boundary(&self) -> Rect {
        Rect::from_origin_size(Point::ORIGIN, self.extent)
    }
}

/// Routes one edge from its source port to its target port, as an arc geometry lifts over the scene.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Route {
    pub(crate) source: Port,
    pub(crate) target: Port,
}

impl Route {
    pub(crate) fn new(source: Port, target: Port) -> Self {
        Self { source, target }
    }
}

/// Marks the port where a route attaches to a side of a footprint or of the level boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Port {
    pub(crate) side: Side,
    pub(crate) point: Point,
}

impl Port {
    pub(crate) fn new(side: Side, point: Point) -> Self {
        Self { side, point }
    }
}

/// Names a side of a plan-space rectangle: north is min y, east max x, south max y, west min x.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Side {
    North,
    East,
    South,
    West,
}

/// Stubs the edges that leave this level toward one external endpoint through one boundary slot.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Stub {
    /// Port on the level boundary; the stubbed routes of this level end or start on it.
    pub(crate) slot: Port,
    pub(crate) edges: BTreeSet<EdgeId>,
}

impl Stub {
    pub(crate) fn new(slot: Port, edges: BTreeSet<EdgeId>) -> Self {
        Self { slot, edges }
    }
}

#[cfg(test)]
mod fixtures {
    use super::place::STUB_MARGIN;
    use super::*;

    impl LevelPlans {
        /// Plan the root level alone, as `root_plan` lays it out.
        pub(crate) fn root_only(root_plan: LevelPlan) -> Self {
            let root = ElementPath::root();
            let plans_by_level = BTreeMap::from([(root, root_plan)]);
            Self::new(plans_by_level)
        }

        /// Return these plans with every stub removed, everything else as laid out.
        pub(crate) fn without_stubs(mut self) -> Self {
            for plan in self.plans_by_level.values_mut() {
                plan.stubs.clear();
            }
            self
        }
    }

    impl LevelPlan {
        /// Plan the root level with boxes `a` and `b` and one route `a -> b`.
        pub(crate) fn two_boxes_and_one_route() -> Self {
            let a = ElementPath::from_segments(["a"]);
            let b = ElementPath::from_segments(["b"]);
            let edge = EdgeId::new(a.clone(), b.clone(), 0);
            let a_footprint = Rect::new(20.0, 30.0, 80.0, 70.0);
            let b_footprint = Rect::new(140.0, 50.0, 200.0, 90.0);
            let source = Port::new(Side::East, Point::new(80.0, 50.0));
            let target = Port::new(Side::West, Point::new(140.0, 70.0));
            let route = Route::new(source, target);
            let extent = Size::new(220.0, 110.0);
            let mut plan = Self::new(extent);
            plan.footprints.insert(a, a_footprint);
            plan.footprints.insert(b, b_footprint);
            plan.routes.insert(edge, route);
            plan
        }

        /// Return the band along `side` that bottom-up sizing keeps free of footprints for stub markers, as deep as the stub margin.
        pub(crate) fn stub_band(&self, side: Side) -> Rect {
            let Rect { x0, y0, x1, y1 } = self.boundary();
            match side {
                Side::North => Rect::new(x0, y0, x1, y0 + STUB_MARGIN),
                Side::East => Rect::new(x1 - STUB_MARGIN, y0, x1, y1),
                Side::South => Rect::new(x0, y1 - STUB_MARGIN, x1, y1),
                Side::West => Rect::new(x0, y0, x0 + STUB_MARGIN, y1),
            }
        }
    }

    impl Port {
        /// Tell whether the port sits on its named side of `rect`.
        pub(crate) fn lies_on_side_of(&self, rect: Rect) -> bool {
            let Point { x, y } = self.point;
            let is_within_x_span = (rect.x0..=rect.x1).contains(&x);
            let is_within_y_span = (rect.y0..=rect.y1).contains(&y);
            match self.side {
                Side::North => y == rect.y0 && is_within_x_span,
                Side::East => x == rect.x1 && is_within_y_span,
                Side::South => y == rect.y1 && is_within_x_span,
                Side::West => x == rect.x0 && is_within_y_span,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_route_starts_and_ends_on_its_boxes_ports() {
        let plan = LevelPlan::two_boxes_and_one_route();
        for (edge, route) in &plan.routes {
            let source_footprint = plan.footprints[edge.from()];
            let target_footprint = plan.footprints[edge.to()];
            assert!(route.source.lies_on_side_of(source_footprint));
            assert!(route.target.lies_on_side_of(target_footprint));
        }
    }
}
