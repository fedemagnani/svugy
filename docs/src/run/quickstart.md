# Quickstart

Install the CLI, render a diagram, open it:

```sh
cargo install --path crates/svugy-cli
svugy render docs/diagrams/pipeline.d2 -o pipeline.svg
open pipeline.svg
```

That's the diagram from the overview. Open it in a browser and click around.

Before rendering something new, validate it. svugy reports every problem at once, and for d2 input each one points at its line. Errors stop a render; warnings don't. Add `--format json` if a script or an agent is reading the output.

```sh
svugy validate my.d2
svugy validate my.d2 --format json
```

`render` writes the interactive SVG by default: themed, animated, explorable. Pass `--profile static` when the file is going somewhere scripts don't run, like an `<img>` tag, a README or Figma. The static profile has no script and draws text as paths, so it looks the same everywhere.

The exit codes are the same for every command: `0` means success (warnings included), `1` means the input has errors, `2` is a usage error, and `3` means a file couldn't be read or written.

`svugy import my.d2 -o my.json` saves the model as JSON, and `render` accepts JSON as well as d2.

Next: [Writing diagrams](writing-diagrams.md)
