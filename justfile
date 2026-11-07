svugy := "target/release/svugy"
figures := "docs/src/figures"
playground := "docs/src/playground"

# List the recipes.
default:
    @just --list

# Render the figures, build the live editor, build the book into docs/book, then open it.
docs: figures playground
    mdbook build docs --open

# Render every docs/diagrams/*.md and *.d2, the shipped example, the golden examples/svugy.md and the Rust web-app example as interactive SVGs.
figures:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --quiet --release -p svugy-cli
    mkdir -p {{figures}}
    for source in docs/diagrams/*.md docs/diagrams/*.d2 crates/svugy-cli/example/webapp.md examples/svugy.md; do
        name="$(basename "${source%.*}")"
        {{svugy}} render "$source" -o "{{figures}}/$name.svg"
    done
    cargo run --quiet --release -p svugy --example webapp > "{{figures}}/webapp-rust.svg"

# Compile svugy to wasm and embed it, base64-encoded, in a script the live editors load, so they work from file:// too.
playground:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --quiet --profile wasm-release --target wasm32-unknown-unknown -p svugy-wasm
    wasm="target/wasm32-unknown-unknown/wasm-release/svugy_wasm.wasm"
    {
        printf 'globalThis.svugyWasmBase64 = "'
        base64 < "$wasm" | tr -d '\n'
        printf '";\n'
    } > "{{playground}}/svugy-wasm.js"

# Render the shipped web-app example, which draws every traveler, as the interactive SVG the runtime is tried against.
fixture:
    cargo run --quiet -p svugy-cli -- render crates/svugy-cli/example/webapp.md -o crates/svugy/runtime/fixture.svg

# Render examples/svugy.d2 as the static SVG the README shows.
readme:
    cargo run --quiet -p svugy-cli -- render examples/svugy.d2 --profile static -o docs/svugy.svg
