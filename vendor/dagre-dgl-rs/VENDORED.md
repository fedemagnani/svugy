# Vendored dagre-dgl-rs

- Upstream: dagre-dgl-rs 0.1.1 from crates.io (https://github.com/rinfimate/dagre-dgl-rs), MIT, LICENSE kept as shipped.
- Why vendored: svugy promises same input → same bytes, and upstream broke ties between Brandes-Köpf alignments in HashMap order and numbered dummy nodes from a process-wide counter.
- Wiring: `[patch.crates-io]` in the root Cargo.toml (the `=0.1.1` requirement stays in `workspace.dependencies`); `workspace.exclude` keeps it out of `cargo fmt --all` and `cargo clippy --workspace`, so the code stays as upstream wrote it.

## Local patches

- `position/bk.rs`: `find_smallest_width_alignment` visits the alignments in the fixed order ul, ur, dl, dr (dagre-js's insertion order) instead of HashMap order.
- `graph.rs`, `util.rs`, `acyclic.rs`, `order/build_layer_graph.rs`: the unique-id counter is a field of `Graph` (`Graph::unique_id`); the static `util::unique_id` and `util::reset_id_counter` are removed.
- `tests.rs`: the unique-id tests call `Graph::unique_id`.
- `data/acyclic.rs`: deleted; it sat outside the module tree (`data/mod.rs` declares only `list`) and called the removed `util::unique_id`.
- `Cargo.toml`: upstream's `Cargo.toml.orig` without `petgraph`, which no source file uses.
- Not copied: `.cargo_vcs_info.json`, `.cargo-ok`, `Cargo.lock`, `.github/`, `.gitignore`.

## Hash-order audit

Every `HashMap`/`HashSet` under `src/` (upstream tests aside), and why its iteration order cannot reach the output:

- `graph.rs`: `in_edges`, `out_edges`, `parent`, `children` are keyed lookups only; nodes, edges and each node's neighbour map are `IndexMap`s; the postorder/preorder visited sets are membership checks.
- Visited/stack maps in `acyclic.rs` (dfs FAS), `order/init_order.rs`, `rank/util.rs` (longest path), `rank/network_simplex.rs` (low/lim) and `position/bk.rs` (`horizontal_compaction`): membership checks only.
- `nesting_graph.rs` `tree_depths`: lookups and a max over the values.
- `parent_dummy_chains.rs` postorder numbers: lookups only.
- `order/mod.rs` `nodes_by_rank`, `order/sort_subgraph.rs` `subgraphs`, `order/add_subgraph_constraints.rs` `prev`, `order/resolve_conflicts.rs` `name_to_idx`, `order/cross_count.rs` `south_pos`: lookups only.
- `position/bk.rs` conflicts: a set, and merging type-2 into type-1 is order-free; `root`, `align`, `pos`: lookups only.
- `position/bk.rs` `horizontal_compaction` walks `align.keys()` copying each block root's x; roots map to themselves, so the walk order is irrelevant.
- `position/bk.rs` `align_coordinates` and `balance`: min/max or a sort before use, results written back by key; `position/mod.rs` writes x by key.
- `position/bk.rs` `find_smallest_width_alignment`: kept the first strictly smallest width over `xss.values()`, so ties followed hash order; patched above.
- `util.rs` `successor_weights`, `predecessor_weights`, `map_values`, `map_values_ref`, `zip_object`: never called by the layout pipeline.
- Dummy and reversed-edge names never order anything (no sort or comparison by name beyond `bk.rs`'s order-free conflict key), so the counter's values cannot move a node.
