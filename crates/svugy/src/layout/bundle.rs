//! Bundle the routes entering containers: the edges joining one pair of a level's children share one trunk between the containers' slots and branch flat on each container's floor.

use std::collections::{BTreeMap, BTreeSet};
use std::iter;

use kurbo::{ParamCurve, Rect, Vec2};
use svugy_model::model::EdgeId;
use svugy_model::path::ElementPath;

use super::edge_levels::HomedEdges;
use super::route::TRACE_CLEARANCE;
use super::trace::OrthogonalTrace;
use super::{Branch, Course, EdgeEnd, LevelRoutes, PlacedLevels, Port, Route, Side};

/// Holds the edges of one level joining the same pair of its children, in edge order, with the trunk slot of every end entering a container.
struct Bundle<'a> {
    edges: Vec<&'a EdgeId>,
    entered_ends: Vec<EnteredEnd>,
}

/// Names a bundle end entering `container`, whose trunk lands on `slot` on its boundary.
struct EnteredEnd {
    end: EdgeEnd,
    container: ElementPath,
    slot: Port,
}

/// Traces branches clear across the floors of one level's child containers, in the level's plan, keeping every kept branch for later ones to avoid.
struct ContainerFloors<'a> {
    placed_levels: &'a PlacedLevels,
    level: &'a ElementPath,
    kept_traces_by_container: BTreeMap<ElementPath, Vec<OrthogonalTrace>>,
}

/// Holds a branch traced clear on its container's floor, with the endpoint port it reaches and its trace.
struct TracedBranch {
    branch: Branch,
    endpoint_port: Port,
    trace: OrthogonalTrace,
}

impl PlacedLevels {
    /// Give every bundle of two or more routes homed in one level, joining the same pair of its children with a container among them, one shared trunk and a flat branch per route on each container's floor.
    pub(super) fn bundle_routes_entering_containers(
        &self,
        homed_edges: &HomedEdges,
        level_routes: &mut LevelRoutes,
    ) {
        for level in self.by_level.keys() {
            let bundled_routes = self.bundled_routes_in(level, homed_edges, level_routes);
            let home_routes = level_routes
                .by_level
                .get_mut(level)
                .expect("every placed level has its routes anchored");
            home_routes.extend(bundled_routes);
        }
    }

    /// Route every bundle homed in `level` through its shared trunk, leaving out the routes that stay as anchored.
    fn bundled_routes_in(
        &self,
        level: &ElementPath,
        homed_edges: &HomedEdges,
        level_routes: &LevelRoutes,
    ) -> Vec<(EdgeId, Route)> {
        let home_routes = &level_routes.by_level[level];
        let mut container_floors = ContainerFloors::new(self, level, level_routes);
        let bundles = self.bundles_in(level, homed_edges);
        bundles
            .iter()
            .flat_map(|bundle| bundle.routes_through_trunk(&mut container_floors, home_routes))
            .collect()
    }

    /// Group the edges homed in `level` by the pair of its children holding their endpoints, split each pair's edges into complete groups, and keep the groups that bundle.
    fn bundles_in<'a>(&self, level: &ElementPath, homed_edges: &HomedEdges<'a>) -> Vec<Bundle<'a>> {
        let child_depth = level.depth() + 1;
        let mut edges_by_lifted_pair: BTreeMap<(ElementPath, ElementPath), Vec<&'a EdgeId>> =
            BTreeMap::new();
        for &edge in homed_edges.homed_in(level) {
            let source = edge.from();
            let target = edge.to();
            let source_child = source.ancestor_or_self_at_depth(child_depth);
            let target_child = target.ancestor_or_self_at_depth(child_depth);
            let lifted_pair = (source_child, target_child);
            let lifted_edges = edges_by_lifted_pair.entry(lifted_pair).or_default();
            lifted_edges.push(edge);
        }
        let mut bundles = Vec::new();
        for (lifted_pair, edges) in edges_by_lifted_pair {
            let complete_groups = Self::complete_groups_of(edges);
            let pair_bundles = complete_groups
                .into_iter()
                .filter_map(|group| self.bundle_of(level, &lifted_pair, group));
            bundles.extend(pair_bundles);
        }
        bundles
    }

    /// Split `edges` into groups joining every source of a group to every target of it, so a drawn bundle never implies an edge that does not exist: one edge per endpoint pair, a parallel edge keeping its own arc, grouped at whichever end needs fewer groups.
    fn complete_groups_of(mut edges: Vec<&EdgeId>) -> Vec<Vec<&EdgeId>> {
        edges.sort();
        edges.dedup_by(|edge, earlier_edge| {
            let is_same_source = edge.from() == earlier_edge.from();
            let is_same_target = edge.to() == earlier_edge.to();
            is_same_source && is_same_target
        });
        let grouped_at_targets = Self::complete_groups_keyed_at(&edges, EdgeEnd::Target);
        let grouped_at_sources = Self::complete_groups_keyed_at(&edges, EdgeEnd::Source);
        let is_sources_grouping_fewer = grouped_at_sources.len() < grouped_at_targets.len();
        if is_sources_grouping_fewer {
            return grouped_at_sources;
        }
        grouped_at_targets
    }

    /// Group `edges` by the set of far endpoints each endpoint at `grouped_end` reaches: the endpoints sharing a set, joined to all of it, form one complete group.
    fn complete_groups_keyed_at<'a>(
        edges: &[&'a EdgeId],
        grouped_end: EdgeEnd,
    ) -> Vec<Vec<&'a EdgeId>> {
        let far_end = grouped_end.opposite();
        let mut far_endpoints_by_endpoint: BTreeMap<&ElementPath, BTreeSet<&ElementPath>> =
            BTreeMap::new();
        for &edge in edges {
            let endpoint = grouped_end.endpoint_of(edge);
            let far_endpoint = far_end.endpoint_of(edge);
            let far_endpoints = far_endpoints_by_endpoint.entry(endpoint).or_default();
            far_endpoints.insert(far_endpoint);
        }
        let mut edges_by_far_endpoints: BTreeMap<&BTreeSet<&ElementPath>, Vec<&'a EdgeId>> =
            BTreeMap::new();
        for &edge in edges {
            let endpoint = grouped_end.endpoint_of(edge);
            let far_endpoints = &far_endpoints_by_endpoint[endpoint];
            let group = edges_by_far_endpoints.entry(far_endpoints).or_default();
            group.push(edge);
        }
        edges_by_far_endpoints.into_values().collect()
    }

    /// Bundle two or more `edges` of one complete group joining the `lifted_pair` of children of `level` when they enter a container among them, every entered endpoint a direct child of it, slotting it where the ray between both children's centres leaves it.
    fn bundle_of<'a>(
        &self,
        level: &ElementPath,
        lifted_pair: &(ElementPath, ElementPath),
        edges: Vec<&'a EdgeId>,
    ) -> Option<Bundle<'a>> {
        let is_lone_edge = edges.len() < 2;
        if is_lone_edge {
            return None;
        }
        let (source_child, target_child) = lifted_pair;
        let child_ends = [
            (EdgeEnd::Source, source_child, target_child),
            (EdgeEnd::Target, target_child, source_child),
        ];
        let mut entered_ends = Vec::new();
        for (end, child, far_child) in child_ends {
            let is_leaf_end = edges.iter().all(|edge| end.endpoint_of(edge) == child);
            if is_leaf_end {
                continue;
            }
            let has_endpoint_below_direct_children = edges.iter().any(|edge| {
                let endpoint = end.endpoint_of(edge);
                let endpoint_level = endpoint.enclosing_level();
                endpoint_level != *child
            });
            if has_endpoint_below_direct_children {
                return None;
            }
            let child_footprint = self.footprint_in_level(child, level);
            let far_footprint = self.footprint_in_level(far_child, level);
            let child_center = child_footprint.center();
            let far_center = far_footprint.center();
            let slot = Port::leaving(child_footprint, child_center, far_center);
            let container = child.clone();
            let entered_end = EnteredEnd {
                end,
                container,
                slot,
            };
            entered_ends.push(entered_end);
        }
        let is_between_leaves = entered_ends.is_empty();
        if is_between_leaves {
            return None;
        }
        Some(Bundle {
            edges,
            entered_ends,
        })
    }
}

impl Bundle<'_> {
    /// Route through the shared trunk every edge whose every entered end branches clear, keeping their branches on `container_floors`, or none when fewer than two do; a leaf end's trunk lands on the first such edge's anchored port.
    fn routes_through_trunk(
        &self,
        container_floors: &mut ContainerFloors,
        home_routes: &BTreeMap<EdgeId, Route>,
    ) -> Vec<(EdgeId, Route)> {
        let branched_edges: Vec<(&EdgeId, BTreeMap<EdgeEnd, TracedBranch>)> = self
            .edges
            .iter()
            .filter_map(|&edge| {
                let traced_branches = self.traced_branches_of(edge, container_floors)?;
                Some((edge, traced_branches))
            })
            .collect();
        let is_bundle_held = branched_edges.len() >= 2;
        if !is_bundle_held {
            return Vec::new();
        }
        let (first_branched_edge, _first_traced_branches) = &branched_edges[0];
        let first_branched_route = &home_routes[*first_branched_edge];
        let leaf_source = first_branched_route.source;
        let leaf_target = first_branched_route.target;
        let mut bundled_routes = Vec::new();
        for (edge, mut traced_branches) in branched_edges {
            let source_traced = traced_branches.remove(&EdgeEnd::Source);
            let target_traced = traced_branches.remove(&EdgeEnd::Target);
            let source = source_traced
                .as_ref()
                .map_or(leaf_source, |traced| traced.endpoint_port);
            let target = target_traced
                .as_ref()
                .map_or(leaf_target, |traced| traced.endpoint_port);
            let source_branch = source_traced.map(|traced| container_floors.keep_branch(traced));
            let target_branch = target_traced.map(|traced| container_floors.keep_branch(traced));
            let course = Course::Bundled {
                source_branch,
                target_branch,
            };
            let route = Route {
                source,
                target,
                course,
            };
            let edge_key = edge.clone();
            bundled_routes.push((edge_key, route));
        }
        bundled_routes
    }

    /// Trace `edge`'s branch at every entered end, or `None` when one finds no clear trace.
    fn traced_branches_of(
        &self,
        edge: &EdgeId,
        container_floors: &ContainerFloors,
    ) -> Option<BTreeMap<EdgeEnd, TracedBranch>> {
        let mut traced_branches = BTreeMap::new();
        for entered_end in &self.entered_ends {
            let endpoint = entered_end.end.endpoint_of(edge);
            let traced_branch = container_floors.trace_branch(
                &entered_end.container,
                entered_end.slot,
                endpoint,
            )?;
            traced_branches.insert(entered_end.end, traced_branch);
        }
        Some(traced_branches)
    }
}

impl<'a> ContainerFloors<'a> {
    /// Trace on the floors of `level`'s child containers, each starting with its own flat routes from `level_routes`.
    fn new(
        placed_levels: &'a PlacedLevels,
        level: &'a ElementPath,
        level_routes: &LevelRoutes,
    ) -> Self {
        let children = placed_levels.by_level[level].footprints();
        let kept_traces_by_container = children
            .iter()
            .filter_map(|(child, child_footprint)| {
                let container_routes = level_routes.level(child)?;
                let container_origin = child_footprint.origin();
                let container_offset = container_origin.to_vec2();
                let flat_traces = Self::flat_traces_of(container_routes, container_offset);
                let container = child.clone();
                Some((container, flat_traces))
            })
            .collect();
        Self {
            placed_levels,
            level,
            kept_traces_by_container,
        }
    }

    /// Trace a branch on `container`'s floor from `slot` on its boundary to the midpoint of a side of `endpoint`, clear of `container`'s other children and of every trace kept there: the side facing the slot, else the side facing the slot's boundary.
    fn trace_branch(
        &self,
        container: &ElementPath,
        slot: Port,
        endpoint: &ElementPath,
    ) -> Option<TracedBranch> {
        let endpoint_footprint = self.placed_levels.footprint_in_level(endpoint, self.level);
        let endpoint_center = endpoint_footprint.center();
        let side_facing_slot = Side::facing(endpoint_center, slot.point);
        let mut endpoint_sides = vec![side_facing_slot, slot.side];
        endpoint_sides.dedup();
        let slot_inward_side = slot.side.opposite();
        let slot_leaving_inward = Port::new(slot_inward_side, slot.point);
        let obstacles = self.obstacles_around(container, endpoint);
        let kept_traces = &self.kept_traces_by_container[container];
        let (endpoint_port, trace) = endpoint_sides.into_iter().find_map(|endpoint_side| {
            let endpoint_side_line = endpoint_side.line_along(endpoint_footprint);
            let endpoint_side_midpoint = endpoint_side_line.eval(0.5);
            let endpoint_port = Port::new(endpoint_side, endpoint_side_midpoint);
            let trace = OrthogonalTrace::find_clear(
                slot_leaving_inward,
                endpoint_port,
                &obstacles,
                kept_traces,
            )?;
            Some((endpoint_port, trace))
        })?;
        let bends = trace.bends();
        let branch_container = container.clone();
        let branch = Branch {
            container: branch_container,
            slot,
            bends,
        };
        Some(TracedBranch {
            branch,
            endpoint_port,
            trace,
        })
    }

    /// Keep `traced`'s trace on its container's floor for later branches to avoid, returning its branch.
    fn keep_branch(&mut self, traced: TracedBranch) -> Branch {
        let TracedBranch {
            branch,
            endpoint_port: _endpoint_port,
            trace,
        } = traced;
        let kept_traces = self
            .kept_traces_by_container
            .get_mut(&branch.container)
            .expect("a branch is kept on the floor it was traced on");
        kept_traces.push(trace);
        branch
    }

    /// Return the footprints of `container`'s children but `endpoint`, in this level's plan, inflated by the trace clearance.
    fn obstacles_around(&self, container: &ElementPath, endpoint: &ElementPath) -> Vec<Rect> {
        let container_footprint = self.placed_levels.footprint_in_level(container, self.level);
        let container_origin = container_footprint.origin();
        let container_offset = container_origin.to_vec2();
        let container_children = self.placed_levels.by_level[container].footprints();
        container_children
            .iter()
            .filter(|&(child, _child_footprint)| child != endpoint)
            .map(|(_child, child_footprint)| {
                let child_footprint_in_level = *child_footprint + container_offset;
                child_footprint_in_level.inflate(TRACE_CLEARANCE, TRACE_CLEARANCE)
            })
            .collect()
    }

    /// Return the traces of the flat `container_routes`, moved by `container_offset` into this level's plan.
    fn flat_traces_of(
        container_routes: &BTreeMap<EdgeId, Route>,
        container_offset: Vec2,
    ) -> Vec<OrthogonalTrace> {
        container_routes
            .values()
            .filter_map(|route| {
                let Course::Flat { bends } = &route.course else {
                    return None;
                };
                let source_point = iter::once(route.source.point);
                let bend_points = bends.iter().copied();
                let target_point = iter::once(route.target.point);
                let local_points = source_point.chain(bend_points).chain(target_point);
                let points = local_points.map(|point| point + container_offset).collect();
                let flat_trace = OrthogonalTrace::through(points);
                Some(flat_trace)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use svugy_model::builder::{ElementRef, ModelBuilder};
    use svugy_model::model::{Edge, Element, PlacementHint};

    use super::*;
    use crate::layout::LevelPlans;
    use crate::metrics::BundledFont;

    #[test]
    fn edges_from_a_leaf_into_one_container_share_a_trunk_and_branch_flat_unless_one_ends_deeper() {
        let mut builder = ModelBuilder::new();
        let site = builder.add_top_level("site", Element::new("site"));
        let client = builder.add_child(&site, "client", Element::new("client"));
        let column_pool = Element::new("pool").with_child_placement(PlacementHint::Column);
        let pool = builder.add_child(&site, "pool", column_pool);
        let alpha = builder.add_child(&pool, "alpha", Element::new("alpha"));
        let beta = builder.add_child(&pool, "beta", Element::new("beta"));
        let deep = builder.add_child(&site, "deep", Element::new("deep"));
        let shallow = builder.add_child(&deep, "shallow", Element::new("shallow"));
        let nest = builder.add_child(&deep, "nest", Element::new("nest"));
        let core = builder.add_child(&nest, "core", Element::new("core"));
        for inner in [&alpha, &beta, &shallow, &core] {
            builder.add_edge(&client, inner, Edge::new());
        }
        let model = builder.build().model.expect("fixture model is valid");
        let font = BundledFont::new();

        let plans = LevelPlans::lay_out(&model, &font);

        let site_plan = plans.level(site.path()).expect("site is a level");
        let site_routes = site_plan.routes();
        let route_to = |inner: &ElementRef| {
            let client_path = client.path().clone();
            let inner_path = inner.path().clone();
            let edge = EdgeId::new(client_path, inner_path, 0);
            site_routes[&edge].clone()
        };
        let alpha_route = route_to(&alpha);
        let beta_route = route_to(&beta);
        let pool_footprint = plans.footprint_in_level(pool.path(), site.path());
        for (inner, route) in [(&alpha, &alpha_route), (&beta, &beta_route)] {
            let inner_path = inner.path();
            let Course::Bundled {
                source_branch: None,
                target_branch: Some(branch),
            } = &route.course
            else {
                panic!("client -> {inner_path} enters pool through one branch: {route:?}");
            };
            let inner_footprint = plans.footprint_in_level(inner_path, site.path());
            assert_eq!(branch.container, *pool.path());
            assert!(branch.slot.lies_on_side_of(pool_footprint));
            assert!(route.target.lies_on_side_of(inner_footprint));
            assert!(
                runs_axis_aligned(branch, route.target),
                "a branch runs flat from its slot to {inner_path}: {branch:?}"
            );
        }
        let trunk_ends_of = |route: &Route| {
            let Course::Bundled {
                target_branch: Some(branch),
                ..
            } = &route.course
            else {
                panic!("a bundled route into pool carries its branch: {route:?}");
            };
            (route.source, branch.slot)
        };
        assert_eq!(
            trunk_ends_of(&alpha_route),
            trunk_ends_of(&beta_route),
            "one bundle shares one trunk: the leaf port and the pool slot"
        );
        for inner in [&shallow, &core] {
            let route = route_to(inner);
            assert_eq!(
                route.course,
                Course::Lifted,
                "a group with an endpoint below a direct child of deep stays unbundled"
            );
        }
    }

    #[test]
    fn a_bundle_never_implies_an_edge_missing_from_the_diagram() {
        let topic_to_consumer = |topic: &str, consumer: &str| {
            let topic_path = ElementPath::from_segments(["broker", topic]);
            let consumer_path = ElementPath::from_segments(["consumers", consumer]);
            EdgeId::new(topic_path, consumer_path, 0)
        };
        let orders_to_notify = topic_to_consumer("orders", "notify");
        let edges = [
            topic_to_consumer("orders", "analytics"),
            topic_to_consumer("orders", "fulfillment"),
            orders_to_notify.clone(),
            topic_to_consumer("captured", "analytics"),
            topic_to_consumer("captured", "fulfillment"),
        ];
        let edge_refs: Vec<&EdgeId> = edges.iter().collect();

        let complete_groups = PlacedLevels::complete_groups_of(edge_refs);

        for group in &complete_groups {
            let sources: BTreeSet<&ElementPath> = group.iter().map(|edge| edge.from()).collect();
            let targets: BTreeSet<&ElementPath> = group.iter().map(|edge| edge.to()).collect();
            let joined_pair_count = sources.len() * targets.len();
            assert_eq!(
                group.len(),
                joined_pair_count,
                "every source of a bundle must reach every target of it: {group:?}"
            );
        }
        let lone_groups: Vec<&Vec<&EdgeId>> = complete_groups
            .iter()
            .filter(|group| group.len() == 1)
            .collect();
        assert_eq!(
            lone_groups,
            [&vec![&orders_to_notify]],
            "captured never reaches notify, so orders -> notify leaves the shared trunk"
        );
    }

    /// Tell whether the branch turns at right angles only, from its slot through its bends to `endpoint_port`.
    fn runs_axis_aligned(branch: &Branch, endpoint_port: Port) -> bool {
        let slot_point = iter::once(branch.slot.point);
        let bend_points = branch.bends.iter().copied();
        let endpoint_point = iter::once(endpoint_port.point);
        let points: Vec<_> = slot_point
            .chain(bend_points)
            .chain(endpoint_point)
            .collect();
        points.windows(2).all(|leg| {
            let is_horizontal = leg[0].y == leg[1].y;
            let is_vertical = leg[0].x == leg[1].x;
            is_horizontal || is_vertical
        })
    }
}
