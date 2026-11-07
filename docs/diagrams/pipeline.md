---
title: svugy · one render
subtitle: from your diagram file to one explorable SVG
styles:
  stage: (color=#3478f6)
  flow: (traveler=parcel)
---

<!-- svugy rendering one diagram, drawn by svugy. -->

## Components
- cli: svugy-cli (caption=svugy render, color=#9945ff, height=60, position=0,60)
- pipeline: svugy (caption=the pipeline, color=#1b2436, position=220,0)
  - layout: Layout (stage, caption=plan space, position=18,18)
  - geometry: Geometry (stage, caption=isometric, position=190,18)
  - render: Render (stage, caption=markup, position=362,18)
  - inter: Inter (shape=cylinder, color=#d5dcea, position=190,200)
- runtime (caption=svugy.js · svugy.css, color=#3a3f4b, position=840,0)

## Edges
- layout -> geometry: boxes (flow)
- geometry -> render: shapes (flow)
- layout -> inter: measures (line=badge)
- render -> inter: outlines (line=badge)
- cli -> layout: ① ② model (traveler=document)
- runtime -> render: ③ (line=dotted, traveler=key)

## Legend: What happens on render
- ① the CLI parses your diagram
- ② the pipeline lays it out and draws it
- ③ the runtime is embedded, the SVG written
- (line=dotted): embedded at build time
- (line=badge, target=inter): reads the bundled font
