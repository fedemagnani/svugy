# Determinism

You commit SVGs and review their diffs, so the same input has to give the same bytes on every machine.

The font is bundled, so label widths never depend on what's installed. `std`'s `sin` and `cbrt` can differ in the last bit between platforms, so every transcendental goes through `libm`, and Clippy bans the `std` versions. Numbers have fixed decimals and attributes are sorted. dagre had a spot where `HashMap` iteration order leaked into the layout; it's vendored and patched.

Snapshot tests pin the model JSON and both SVG profiles, and a pixel golden pins the static raster. Three hundred random graphs are each rendered twice and compared. CI renders the same input on Linux, macOS and wasm and diffs the bytes.

The banned functions are in `clippy.toml`, every dagre patch is listed in `vendor/svugy-dagre/VENDORED.md`, the tests are in `crates/svugy/tests/determinism.rs`, and the CI jobs are in `.github/workflows/ci.yml`.
