# Render

Render writes a `Scene` as SVG text. Both outputs come from one `Renderer<P>`, so the static and the interactive SVG draw the same scene; only the profile `P` differs.

## Components

**Writer** produces valid, injection-free markup.

- Numbers have at most 3 decimals; attributes are sorted; text is XML-escaped.
- CSS and JavaScript go in comment-guarded CDATA, since entity escaping breaks `<style>` and `<script>`.
- Links pass only as `http`, `https` or `mailto` URLs or relative paths.
- CSS selectors are built from static attribute names only; paths travel only as attribute values.

**Theme** holds the colours.

- A dark and a light palette; the light one mirrors the dark in Oklab lightness, L ↦ 1.13 − L.
- A solid's three face shades derive from one base in Oklab: top = base, left = 0.835·base, right = 0.72·base.

**Profiles** are the two flavours of the renderer.

| | `Static` | `Interactive` |
|---|---|---|
| for | viewers without CSS, script or fonts | browsers |
| colours | dark palette, inlined | CSS custom properties, light under `prefers-color-scheme: light` |
| labels | glyph outlines from the bundled Inter | `<text>`, `textLength` pinned to the measured width |
| content | the whole model | every layer, one `<g data-svugy-layer>` each |
| extras | none | stub overlays, travelers, the embedded [runtime](runtime.md) |

- Both write the `data-svugy-*` attributes the runtime reads.
- The interactive file is sized to the canvas, the union of every layer's root view, and shows the deepest layer, so without script it shows the whole diagram.
- Each layer is painted from its own model, so a closed container takes a leaf's fill, not a slab's.

**Travelers** are `<symbol>`s in `<defs>`, one per traveler the file uses, moved along each arc by SMIL every 2.4 s.

**`RuntimeContract`** spells every `data-svugy-*` attribute once; the [runtime contract](runtime-contract.md) defines them.

Code: `crates/svugy/src/render/`.
