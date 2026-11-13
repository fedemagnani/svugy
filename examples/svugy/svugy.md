---
styles:
  lang: (color=#f0a020)
  hub: (color=#14f195)
  stage: (color=#1f3566)
  step: (color=#3478f6)
---

## Components
- text: your text (caption=Markdown · d2 · JSON, color=#3a3f4b)
- cli: svugy-cli (caption=render · validate · import, color=#9945ff, height=70, link=pipeline)
- d2: d2 v0.9.0 (caption=reference grammar, color=transparent, line=dashed, stroke=#5b6170, link=https://d2lang.com)
- import: svugy-parse (caption=text → model, color=#2a2418)
  - markdown: Markdown (lang, caption=parse · print)
  - d2 (lang, caption=import)
  - json: JSON (lang, caption=load · save)
- model: svugy-model (caption=the hub, color=#17271f)
  - path (hub, caption=ids)
  - graph: model (hub, caption=elements · edges)
  - validate (hub, caption=problems · bounds)
- pipeline: svugy (caption=the pipeline, color=#1b2436)
  - metrics (step, caption=measures labels)
  - inter: Inter (shape=cylinder, color=#d5dcea)
  - layout (stage, caption=plan space)
    - place (step, caption=dagre · taffy · pins)
    - route (step, caption=ports)
    - stub (step, caption=boundary slots)
  - geometry (stage, caption=screen space)
    - projection (step, caption=exact constants)
    - primitive (step, caption=4 primitives)
    - depth (step, caption=painter's order)
  - render (stage, caption=markup)
    - theme (step, caption=tokens · Oklab)
    - profile (step, caption=static · interactive)
    - writer (step, caption=escaping · decimals)
- contract: runtime contract (caption=data-svugy-*, shape=slab, color=#334566)
- runtime (caption=svugy.js · svugy.css, color=#3a3f4b)
- svg: one .svg file (shape=disc, color=#d5dcea)

## Workflows
### main: svugy · how it is built (subtitle=text in → model → layout → geometry → SVG out)
[Your text](#text) goes in; [one SVG file](#svg) comes out, with [the runtime](#runtime) inside it.

- text (position=0,40)
- cli (position=0,330)
- d2 (position=260,-220)
- import (position=260,0)
- markdown (position=18,18)
- import.d2 (position=150,18)
- json (position=282,18)
- model (position=260,330)
- path (position=18,18)
- graph (position=150,18)
- validate (position=18,180)
- pipeline (position=760,0)
- metrics (position=600,36)
- inter (position=780,54)
- layout (position=36,36)
- place (position=18,18)
- route (position=150,18)
- stub (position=282,18)
- geometry (position=36,300)
- projection (position=18,18)
- primitive (position=150,18)
- depth (position=282,18)
- render (position=560,300)
- theme (position=18,18)
- profile (position=150,18)
- writer (position=282,18)
- contract (position=1560,760)
- runtime (position=1900,620)
- svg (position=1900,330)

#### model: ① Your text becomes a model
[The CLI](#cli) hands your text to [svugy-parse](#import), which reads Markdown, d2 or JSON into [svugy-model](#model); the d2 importer follows [the d2 grammar](#d2->import). [Validate](#validate) checks the model before anything is drawn.
- text -> cli -> import
- markdown
- import.d2
- json
- d2 -> import: grammar (line=dashed, color=#5b6170)
- import -> model: ①
- path -> graph -> validate

#### layout: ② The model becomes boxes in plan space
[The pipeline](#pipeline) measures every label with [Inter](#inter), the bundled font. Then [place](#place) puts the boxes, [route](#route) anchors each edge on a port and [stub](#stub) picks where edges leave their container.
- cli -> pipeline
- model -> pipeline: ②
- metrics -> layout
- metrics -> inter: measures (line=badge)
- place -> route -> stub

#### render: ③ Boxes become shapes, ④ shapes one SVG file
[The geometry](#geometry) turns each box into an isometric solid and orders the solids for painting; [the renderer](#render) themes them and writes [one SVG file](#pipeline->svg).
- layout -> geometry: ③
- geometry -> render
- projection -> primitive -> depth
- theme -> profile -> writer
- profile -> inter: outlines (line=badge)
- pipeline -> svg: ④

#### runtime: The runtime ships inside the file
[The writer](#writer) emits the `data-svugy-*` attributes of [the runtime contract](#contract). [The runtime](#runtime), embedded at build time, reads them inside [the SVG](#svg) to make it explorable.
- writer -> contract: emits (line=dotted)
- runtime -> contract: reads (line=dotted)
- runtime -> svg (line=dotted)
