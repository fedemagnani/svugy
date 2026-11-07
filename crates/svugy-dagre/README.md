# svugy-dagre

Lays out a directed graph in ranks that advance left to right: node sizes and `(from, to)` index pairs in, node centres out.

```rust
use svugy_dagre::{LeftToRightLayout, Size};

let layout = LeftToRightLayout { node_gap: 48.0, rank_gap: 72.0 };
let node_size = Size { width: 100.0, height: 40.0 };
let centres = layout.node_centres(&[node_size; 2], &[(0, 1)]);
assert!(centres[0].x < centres[1].x);
```

- Algorithm: [dagre-js](https://github.com/dagrejs/dagre)'s pipeline (cycle breaking by depth-first search, network-simplex ranking, barycenter ordering, Brandes-Köpf positioning), cut down from [dagre-dgl-rs](https://github.com/rinfimate/dagre-dgl-rs) 0.1.1 (MIT, `LICENSE`).
- Scope: no compound nodes, edge labels, edge routes or other rank directions; self-loops are ignored and repeated edges count once.
- Determinism: the same input gives the same bits.
  - No global state: dummy-node and reversed-edge ids come from a counter owned by each layout's graph.
  - Brandes-Köpf ties in width go to the first of the alignments ul, ur, dl, dr.
  - `HashMap`s and `HashSet`s are only looked up, or reduced by min, max, a sort or a write by key, so their iteration order never reaches the output.
