svugy := "target/release/svugy"
figures := "docs/src/figures"
playground := "docs/src/playground"

# List the recipes.
default:
    @just --list

# Render the figures, build the live editor, build the book into docs/book, then open it.
docs: figures playground
    mdbook build docs --open

# Render every docs/diagrams/*.d2 and the golden examples/svugy.d2 as interactive SVGs.
figures:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --quiet --release -p svugy-cli
    mkdir -p {{figures}}
    for source in docs/diagrams/*.d2 examples/svugy.d2; do
        name="$(basename "$source" .d2)"
        {{svugy}} render "$source" -o "{{figures}}/$name.svg"
    done

# Compile svugy to wasm and embed it, base64-encoded, in a script the overview loads, so the live editor works from file:// too.
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
