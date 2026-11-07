# Model

Without a shared model, every language would repeat validation and every stage would read every language. `svugy-model` defines the `Model` all parsers produce and all stages read, and validates it once.

## Components

**`ElementPath`** is an element's only identity, in the model and in the SVG.

- Segments from the top down, dot-joined: `pipeline.inter`; a segment holding `.`, `"` or `\` is quoted. The root is the empty path.
- Depth = segment count: root 0, top-level element 1.

**`Model`** holds the diagram.

- `elements: BTreeMap<ElementPath, Element>`. An element with children is a container; there is no container flag.
- `edges: BTreeMap<EdgeId, Edge>`. `EdgeId` is `(from, to, ordinal)`, shown `(a -> b)[0]`; the ordinal keeps parallel edges apart.
- A badge edge is drawn as a mini of its target on its source's top face instead of an arc.
- An edge's home level, where layout places it, is its endpoints' lowest common ancestor.
- Fields are crate-private: outside `svugy-model`, a non-empty `Model` exists only after admission.

**`ModelBuilder`** builds a model through handles.

- `add_top_level` and `add_child` return an `ElementRef`; parents and edge endpoints are `ElementRef`s, so they exist by construction.
- `add_edge` assigns the pair's next free ordinal.
- Markdown and d2 lower through it; Rust programs call it directly ([Rust](../run/rust.md)).

**Admission** (`UncheckedModel::admit`) rejects unsound models.

- Returns `ModelAdmission { model, report }`; `model` is `None` if any problem is an error. Warnings never block.
- `Problem` (`problem.rs`) lists all 17 problem kinds, from dangling references and duplicate paths to unknown styles and badges on containers.
- Every position, height and numeric style is bounded by `MAX_MAGNITUDE` = 1e6, so no valid model yields a huge SVG.
- Unknown names get "did you mean": the closest candidate by case-insensitive Levenshtein distance, if ≤ ⌊len / 3⌋.

**`DisclosedModel`** cuts the model into the layers the interactive SVG steps through.

- Layers 0 to D, D being the depth of the deepest container. Layer D is the model.
- Layer K keeps elements of depth ≤ K + 1. Containers at depth K + 1 lose their children, becoming leaves; `closed_containers` lists them.
- An edge with a hidden endpoint moves to that endpoint's shown ancestor; it is dropped if both ends land on one element.
- Moved edges between one ordered pair merge into one, under the pair's next free ordinal; `lifted_origins` lists what it stands for.
- Layout reads a layer as a plain `Model`; geometry and render also read `closed_containers` and `lifted_origins`.

Example: elements `a`, `a.x`, `a.y`, `b`, edges `a.x -> b` and `a.y -> b`. Layer 0 holds `a` (closed) and `b`, joined by `(a -> b)[0]`, standing for both edges. Layer 1 is the model.

Code: `crates/svugy-model/src/`.
