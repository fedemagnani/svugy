# Render

Render turns the scene into SVG text.

The writer guarantees a valid, safe file. Numbers have fixed decimals, attributes are sorted, and text is XML-escaped. CSS and JavaScript are wrapped in comment-guarded CDATA instead of escaped, because escaping inside `<style>` or `<script>` breaks them. Links pass through a sanitiser that only lets safe URLs out. Your text never ends up in a CSS selector; element paths only travel as attribute values.

The theme holds colour tokens for light and dark, and each face's three shades are derived in Oklab.

There are two output profiles, built by one renderer: `Renderer<Static>` and `Renderer<Interactive>`. The static profile inlines every colour, draws text as glyph outlines and includes no script, so it looks identical in resvg, Figma or an `<img>` tag. Inlining matters: resvg draws `var(--x)` as black, which looks plausible and is wrong. The interactive profile uses CSS tokens, real `<text>`, animated flows and the embedded runtime. Both emit the `data-svugy-*` attributes the runtime reads.

The code is in `crates/svugy/src/render`: `writer/`, `theme/`, `profile/`, `contract.rs`.
