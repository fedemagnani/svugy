svugy := "target/release/svugy"
figures := "docs/src/figures"
playground := "docs/src/playground"

# List the recipes.
default:
    @just --list

# Render the figures, build the live editor, build the book into docs/book, then open it.
docs: figures playground
    mdbook build docs --open

# Assemble the svugy.dev site into _site: the landing page with the book's live editor, each committed example's .md and .svg under models/ beside its sketch render, Excalifont under fonts/, the book under docs/, the released installation prompt as install.md.
site: figures playground
    #!/usr/bin/env bash
    set -euo pipefail
    mdbook build docs
    rm -rf _site
    mkdir -p _site/models
    cp -R site/. _site/
    cp -R docs/book _site/docs
    # mdBook renames its CSS with a content hash; its scripts keep their names, so the page loads those from docs/
    cp docs/theme/playground.css _site/
    # each example also pre-rendered in the sketch theme, shown to a visitor who picked it until the live renderer loads
    for source in examples/*/*.md; do
        cp "$source" "${source%.md}.svg" _site/models/
        name="$(basename "${source%.md}")"
        {{svugy}} render "$source" --theme sketch -o "_site/models/$name-sketch.svg"
    done
    mkdir -p _site/fonts
    cp crates/svugy/assets/fonts/Excalifont-Regular.woff2 crates/svugy/assets/fonts/Excalifont-OFL.txt _site/fonts/
    # the installation prompt the latest release publishes, kept only if our release workflow built it from a commit merged to main
    gh release download --repo fedemagnani/svugy --pattern install.md --output _site/install.md
    source_commit="$(gh attestation verify _site/install.md --repo fedemagnani/svugy \
        --signer-workflow fedemagnani/svugy/.github/workflows/svugy-cli-release.yml \
        --format json --jq '.[0].verificationResult.signature.certificate.sourceRepositoryDigest')"
    git merge-base --is-ancestor "$source_commit" origin/main

# Build the site, then serve _site at http://127.0.0.1:<port> until Ctrl-C; rerun `just site` in another shell to rebuild.
serve-site port="8000": site
    @echo "svugy.dev preview: http://127.0.0.1:{{port}}"
    python3 -m http.server {{port}} --bind 127.0.0.1 --directory _site

# Render every docs/diagrams/*.md, the web-app example in Markdown, d2, Rust and the sketch theme, and the golden examples/svugy/svugy.md as interactive SVGs.
figures:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --quiet --release -p svugy-cli
    mkdir -p {{figures}}
    for source in docs/diagrams/*.md examples/webapp/webapp.md examples/svugy/svugy.md; do
        name="$(basename "${source%.*}")"
        {{svugy}} render "$source" -o "{{figures}}/$name.svg"
    done
    {{svugy}} render examples/webapp/webapp.d2 -o "{{figures}}/webapp-in-d2.svg"
    {{svugy}} render examples/webapp/webapp.md --theme sketch -o "{{figures}}/webapp-sketch.svg"
    cargo run --quiet --release -p svugy-examples --example webapp > "{{figures}}/webapp-rust.svg"

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

# Re-render each committed examples/*/*.svg from the .md beside it.
examples:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --quiet --release -p svugy-cli
    for source in examples/*/*.md; do
        {{svugy}} render "$source" -o "${source%.md}.svg"
    done

# Render the web-app example, which draws every traveler, as the interactive SVG the runtime is tried against.
fixture:
    cargo run --quiet -p svugy-cli -- render examples/webapp/webapp.md -o crates/svugy/runtime/fixture.svg

# Render examples/svugy/svugy.d2 as the static SVG the README shows.
readme:
    cargo run --quiet -p svugy-cli -- render examples/svugy/svugy.d2 --profile static --workflow main -o docs/svugy.svg
