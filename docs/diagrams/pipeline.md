---
styles:
  stage: (color=#3478f6)
  flow: (traveler=parcel)
---

<!-- svugy rendering one diagram, drawn by svugy. -->

## Components
- cli: svugy-cli (caption=svugy render, color=#9945ff, height=60)
- pipeline: svugy (caption=the pipeline, color=#1b2436)
  - layout: Layout (stage, caption=plan space)
  - geometry: Geometry (stage, caption=isometric)
  - render: Render (stage, caption=markup)
  - inter: Inter (shape=cylinder, color=#d5dcea)
- runtime (caption=svugy.js · svugy.css, color=#3a3f4b)

## Workflows
### render: svugy · one render (subtitle=from your diagram file to one explorable SVG)
What happens between `svugy render` reading [your diagram](#cli) and writing one SVG.

- cli (position=0,60)
- pipeline (position=220,0)
- layout (position=18,18)
- geometry (position=190,18)
- render (position=362,18)
- inter (position=190,200)
- runtime (position=840,0)

#### parse: The CLI parses your diagram
[The CLI](#cli) turns your file into a model and hands it to [the layout](#layout).
- cli -> layout: model (traveler=document)

#### draw: The pipeline lays it out and draws it
[The layout](#layout) places boxes in plan space, [the geometry](#geometry) projects them and [the render](#render) writes the markup. Layout measures labels and render outlines text with [Inter](#inter), the bundled font.
- layout -> geometry: boxes (flow)
- geometry -> render: shapes (flow)
- layout -> inter: measures (line=badge)
- render -> inter: outlines (line=badge)

#### embed: The runtime is embedded
Every interactive SVG carries [the runtime](#runtime), bundled into the binary at build time.
- runtime -> render: embedded (line=dotted, traveler=key)
