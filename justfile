svugy := "target/release/svugy"
figures := "docs/src/figures"
playground := "docs/src/playground"

# List the recipes.
default:
    @just --list

# Render the figures, build the live editor, build the book into docs/book, then open it.
docs: figures playground
    mdbook build docs --open

# Render every docs/diagrams/*.md and *.d2, the shipped example and the golden examples/svugy.md as interactive SVGs.
figures:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --quiet --release -p svugy-cli
    mkdir -p {{figures}}
    for source in docs/diagrams/*.md docs/diagrams/*.d2 crates/svugy-cli/example/webapp.md examples/svugy.md; do
        name="$(basename "${source%.*}")"
        {{svugy}} render "$source" -o "{{figures}}/$name.svg"
    done

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

# Three-way merge upstream dagre-dgl-rs <version> into vendor/svugy-dagre/src, based on the recorded upstream version, then record <version>.
dagre-sync version:
    #!/usr/bin/env bash
    set -euo pipefail
    fork="vendor/svugy-dagre"
    base_version="$(sed -n 's/^dagre-dgl-rs = "\(.*\)"$/\1/p' "$fork/Cargo.toml")"
    downloads="$(mktemp -d)"
    for version in "$base_version" "{{version}}"; do
        curl -fsSL -A "svugy-dagre-sync" "https://static.crates.io/crates/dagre-dgl-rs/dagre-dgl-rs-$version.crate" | tar -xz -C "$downloads"
    done
    base="$downloads/dagre-dgl-rs-$base_version"
    theirs="$downloads/dagre-dgl-rs-{{version}}"
    conflicted=()
    for file in $( (cd "$base/src" && find . -name '*.rs'; cd "$theirs/src" && find . -name '*.rs') | sort -u ); do
        ours_file="$fork/src/$file"
        base_file="$base/src/$file"
        theirs_file="$theirs/src/$file"
        is_upstream_changed=true
        if [ -f "$base_file" ] && [ -f "$theirs_file" ] && cmp -s "$base_file" "$theirs_file"; then is_upstream_changed=false; fi
        if [ "$is_upstream_changed" = false ]; then continue; fi
        if [ ! -f "$theirs_file" ]; then rm -f "$ours_file"; echo "removed upstream: $file"; continue; fi
        if [ ! -f "$base_file" ]; then mkdir -p "$(dirname "$ours_file")"; cp "$theirs_file" "$ours_file"; echo "added upstream: $file"; continue; fi
        if [ ! -f "$ours_file" ]; then conflicted+=("$file (deleted here, changed upstream)"); continue; fi
        git merge-file -L svugy-dagre -L "dagre-dgl-rs $base_version" -L "dagre-dgl-rs {{version}}" "$ours_file" "$base_file" "$theirs_file" || conflicted+=("$file")
    done
    perl -pi -e 's/^dagre-dgl-rs = ".*"$/dagre-dgl-rs = "{{version}}"/' "$fork/Cargo.toml"
    diff -u "$base/Cargo.toml.orig" "$theirs/Cargo.toml.orig" || echo "upstream Cargo.toml changed: carry it over by hand"
    if [ "${#conflicted[@]}" -gt 0 ]; then printf 'conflict: %s\n' "${conflicted[@]}"; exit 1; fi
