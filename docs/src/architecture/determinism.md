# Determinism

SVGs are committed and reviewed as diffs, so the same input must give the same bytes on every machine.

## Components

**Pinned inputs**

- Inter is bundled, so label widths never depend on installed fonts.
- `std` transcendentals can differ in the last bit across platforms, so every transcendental goes through `libm`; `clippy.toml` bans the `std` versions and the kurbo calls that use them.
- dagre-dgl-rs broke ties in `HashMap` order and numbered nodes from a global counter; `svugy-dagre`, cut down from it, breaks ties in a fixed order, keeps the counter per graph, and lets no `HashMap` order reach a coordinate (`crates/svugy-dagre/README.md`).
- `Model` and `LevelPlans` keep their entries in `BTreeMap`s, iterated in key order.

**One output spelling**: at most 3 decimals per number, attributes sorted.

**Tests**

- Snapshots pin both SVGs of `examples/svugy.md` and its static raster (`crates/svugy/tests/golden_svugy.rs`).
- 300 random graphs are each rendered twice and compared (`crates/svugy/tests/determinism.rs`).
- CI renders `examples/svugy.d2` on Linux, macOS and wasm32-wasip1 and diffs each result against the committed `docs/svugy.svg`.
