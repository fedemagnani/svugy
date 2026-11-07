# Layout

Layout decides where things go on a flat plane called plan space; projection happens later, in geometry.

It starts by measuring every label with the bundled Inter font. Weights are resolved with CSS rules (450 becomes 500), not fontdb's, so the face that gets measured is the face that gets drawn.

Containers are laid out from the inside out. A container's children are placed first, then the container becomes one box in its parent. Pinned elements go exactly where you put them. Connected children get a layered layout from dagre, and grid, row or column hints are packed with taffy.

An edge between two containers is placed at their lowest common ancestor, as if it ran between the two containers. Each edge leaves its box on the side facing the other end. Where an edge crosses out of a container, a stub slot marks the spot on that side; you see it once you step inside.

An interactive render lays out each disclosed layer on its own. A closed container is a leaf of its layer's model, so it is placed like one, and layout knows nothing about layers. Pins hold in every layer, so a closed container that is smaller than its open self leaves a gap around it in a shallow layer.

Auto layout turns a chain of edges into a long diagonal. Pin elements to control the shape; the map of this book is pinned.

The code is in `crates/svugy/src/metrics` and `crates/svugy/src/layout`: `place/`, `route.rs`, `stub.rs`, `nest.rs`.
