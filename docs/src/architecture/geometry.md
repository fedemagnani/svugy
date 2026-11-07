# Geometry

Geometry turns the flat boxes from layout into isometric shapes.

The projection uses exact constants, so there's no trigonometry at render time. There are four primitives: box, slab, cylinder and disc. Open containers are always slabs. A closed container is drawn like a leaf, with its own shape and height, and gets a sheet: its body's outline a fifth of a block thick, lowered just under it and painted faintly before its faces, as a hint that there is more inside. Solids are sorted back to front, the painter's algorithm, and every connection is drawn as an arc over the scene from port to port.

Labels are kept off faces and off each other. Stub plates lie flat on their slab, badges sit on top faces, and the title and legend stay fixed in document space. Every container gets its own coordinate frame, which is what lets the viewer zoom into it.

Stubs and badges take no ports and route nothing, so hiding them never moves a visible arc.

The code is in `crates/svugy/src/geometry`: `projection.rs`, `primitive.rs`, `level.rs`, `depth.rs`, `connection.rs`, `stub.rs`, `badge.rs`, `obstruction.rs`.
