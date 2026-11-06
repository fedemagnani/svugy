# svugy

Render diagrams as isometric 3D svgs.

![svugy's own architecture, drawn by svugy from examples/svugy.d2](docs/svugy.svg)


## Quickstart

```sh
cargo install --path crates/svugy-cli
svugy render examples/svugy.d2 -o svugy.svg
open svugy.svg
```

- `svugy validate my.d2` reports every problem at once, each pointing at its d2 line; add `--format json` when a script or an agent reads it.
- `svugy render --profile static` writes an SVG for places scripts don't run: `<img>` tags, READMEs, Figma.
- `svugy import my.d2 -o my.json` saves the model as JSON, which `render` accepts too.

The output is one SVG file with its CSS and JavaScript inside: no server, no library to install. The same input renders the same bytes on every platform, wasm included.

## Crates

| Crate | Role |
| --- | --- |
| [`svugy-model`](crates/svugy-model) | the model, its builder, JSON and validation |
| [`svugy-import`](crates/svugy-import) | d2 into the model |
| [`svugy`](crates/svugy) | the pipeline: metrics, layout, geometry, render |
| [`svugy-cli`](crates/svugy-cli) | the `svugy` command |
| [`svugy-wasm`](crates/svugy-wasm) | svugy in the browser, behind the docs playground |
| [`svugy-dagre`](vendor/svugy-dagre) | dagre-dgl-rs, patched for deterministic layout |

## Docs

`just docs` builds the book with a live playground, svugy compiled to wasm re-rendering as you type, and opens it. The sources are in [`docs/src`](docs/src).

## License

Licensed under either of MIT or Apache-2.0, at your option. `svugy-dagre` keeps upstream's MIT license.
