# Geometry

Geometry is the only layer that knows the picture is isometric. It projects `LevelPlans` into a `Scene` in paint order, so render writes it in sequence.

## Components

**Projection**

- screen = ((x − y)·√3/2, (x + y)/2 − z), √3/2 stored as a literal, so no trigonometry runs.
- Each level gets its own frame, mapping it into its parent, and its own view, the rectangle the runtime zooms to.

**Primitives**

- Footprints extrude into a block (leaves, default 22.5 tall), slab (containers, 10.5), cylinder (25.5) or disc (10.5).
- A closed container is drawn as a leaf over a faint 4.5-unit sheet hinting at its contents.

**Depth** sorts each level's solids back to front: a footprint paints first when a sight line x − y = k meets it before the other.

**Connections** are quadratic Béziers from port to port, the apex 20 + 0.2·span plan units above the higher port, so long arcs clear what they cross.

**Plates** put each connection label, stub tag and badge at the candidate spot overlapping the fewest labels and earlier plates.

**Stubs and badges** route nothing.

- A stub tag lies flat on its container's slab by its slot.
- A badge is its target shrunk to 19.2 screen units on its source's top face.
- Adding or removing either moves no arc (tested in `crates/svugy/src/lib.rs`).

**Heading and legend** sit in document space, outside every level's frame.

Code: `crates/svugy/src/geometry/`.
