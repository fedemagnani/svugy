# Layout

Placement is a 2D problem, so layout solves it on the ground plane (plan space) and never projects; geometry projects and never places. `LevelPlans::lay_out` returns each level's footprints, routes and stubs.

## Components

**Metrics**

- Labels are measured in the bundled Inter faces, picked by CSS weight matching: 450 uses the 500 face.
- Layout sizes footprints and geometry places labels from the same measurements.

**Placement** fills each level, innermost first.

- A container is sized by its placed interior; a leaf by its labels, at least 72 × 66.
- Children with a `position` go exactly there; the rest are placed at least 18 units clear of them.
- Default: dagre ranks along plan x, so a chain of edges becomes a screen diagonal. Pin elements to control the shape, as `examples/svugy.md` does.
- `grid`, `row`, `column`: taffy packs the children in path order.

**Routing**

- An edge is routed in its home level between its true endpoints; for ranking, it joins the two children of that level that hold them.
- Each end attaches on the footprint side facing the other end.
- Badge edges get no route.

**Stubs** mark where edges leave a container: one boundary slot per external endpoint, on the side most of its edges cross, at their mean crossing point.

**Layers**

- Each [layer](model.md) is laid out on its own; a closed container is placed as the leaf it is in its layer.
- Pins hold in every layer, so a closed container leaves a gap where its open self would fit.

Code: `crates/svugy/src/metrics/`, `crates/svugy/src/layout/`.
