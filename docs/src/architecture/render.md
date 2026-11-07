# Render

Render turns the scene into SVG text.

The writer guarantees a valid, safe file. Numbers have fixed decimals, attributes are sorted, and text is XML-escaped. CSS and JavaScript are wrapped in comment-guarded CDATA instead of escaped, because escaping inside `<style>` or `<script>` breaks them. Links pass through a sanitiser that only lets safe URLs out. Your text never ends up in a CSS selector; element paths only travel as attribute values.

The theme holds colour tokens for light and dark, and each face's three shades are derived in Oklab.

There are two output profiles, built by one renderer: `Renderer<Static>` and `Renderer<Interactive>`. The static profile inlines every colour, draws text as glyph outlines and includes no script, so it looks identical in resvg, Figma or an `<img>` tag. Inlining matters: resvg draws `var(--x)` as black, which looks plausible and is wrong. The interactive profile uses CSS tokens, real `<text>`, animated flows and the embedded runtime. Both emit the `data-svugy-*` attributes the runtime reads.

The interactive profile writes every disclosed layer into one document. Each layer is laid out and projected on its own, then written as its own `<g data-svugy-layer>` group; the layers share one `<defs>` and one canvas that covers every layer's root view. The document is sized to layer 0, the only layer shown until the runtime switches, so an `<img>` shows layer 0. Paints are read from each layer's model, so a closed container takes a leaf's fill, not a slab's. The static profile writes only the last layer, which is the whole model, with no layer group around it.

Each animated connection carries a traveler: a `<use>` of a `<symbol>` from `<defs>`, moved along the arc by SMIL and painted like its edge. The model's traveler becomes a symbol type in one place, `profile/traveler.rs`, the same way a shape becomes a primitive, and only the symbols a document uses are written. The static profile has no travelers.

The code is in `crates/svugy/src/render`: `writer/`, `theme/`, `profile/`, `contract.rs`.
