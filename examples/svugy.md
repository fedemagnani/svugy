---
title: svugy · how it is built
subtitle: text in → model → layout → geometry → SVG out
styles:
  lang: (color=#f0a020)
  hub: (color=#14f195)
  stage: (color=#1f3566)
  step: (color=#3478f6)
---

<!-- The golden: svugy drawn by svugy, the twin of examples/svugy.d2 in the svugy language. -->

## Components
- text: "your text" (caption="d2 · JSON · Rust builder", color=#3a3f4b, position=0,40)
- cli: "svugy-cli" (caption="render · validate · import", color=#9945ff, position=0,330, height=70, link=pipeline)
- d2: "d2 v0.9.0" (caption="reference grammar", color=transparent, line=dashed, stroke=#5b6170, position=260,-220, link=https://d2lang.com)
- import: "svugy-import" (caption="d2 → model", color=#2a2418, position=260,0)
  - lex (lang, caption="logos tokens", position=18,18)
  - parse (lang, caption="chumsky + recovery", position=150,18)
  - lower (lang, caption="AST → model", position=282,18)
- model: "svugy-model" (caption="the hub", color=#17271f, position=260,330)
  - path (hub, caption="ids", position=18,18)
  - graph: "model" (hub, caption="elements · edges", position=150,18)
  - validate (hub, caption="problems · bounds", position=18,180)
  - json (hub, caption="id-keyed schema", position=150,180)
- pipeline: "svugy" (caption="the pipeline", color=#1b2436, position=760,0)
  - metrics (step, caption="measures labels", position=600,36)
  - inter: "Inter" (shape=cylinder, color=#d5dcea, position=780,54)
  - layout (stage, caption="plan space", position=36,36)
    - place (step, caption="dagre · taffy · pins", position=18,18)
    - route (step, caption="ports", position=150,18)
    - stub (step, caption="boundary slots", position=282,18)
  - geometry (stage, caption="screen space", position=36,300)
    - projection (step, caption="exact constants", position=18,18)
    - primitive (step, caption="4 primitives", position=150,18)
    - depth (step, caption="painter's order", position=282,18)
  - render (stage, caption="markup", position=560,300)
    - theme (step, caption="tokens · Oklab", position=18,18)
    - profile (step, caption="static · interactive", position=150,18)
    - writer (step, caption="escaping · decimals", position=282,18)
- contract: "runtime contract" (caption="data-svugy-*", shape=slab, color=#334566, position=1560,760)
- runtime (caption="svugy.js · svugy.css", color=#3a3f4b, position=1900,620)
- svg: "one .svg file" (shape=disc, color=#d5dcea, position=1900,330)

## Edges
- text -> cli -> import
- cli -> pipeline
- import -> model: "①"
- model -> pipeline: "②"
- pipeline -> svg: "④"
- d2 -> import: "grammar" (line=dashed, color=#5b6170, traveler=document)
- runtime -> svg (line=dotted)
- runtime -> contract: "reads" (line=dotted)
- writer -> contract: "emits" (line=dotted, traveler=envelope)
- lex -> parse -> lower
- path -> graph -> validate
- graph -> json
- metrics -> layout
- layout -> geometry: "③"
- geometry -> render
- metrics -> inter: "measures" (line=badge)
- profile -> inter: "outlines" (line=badge)
- place -> route -> stub
- projection -> primitive -> depth
- theme -> profile -> writer

## Legend: Read it as a story
- ① your text becomes a model
- ② the model becomes boxes in plan space
- ③ boxes become isometric shapes
- ④ shapes become one SVG file, runtime inside
- (line=dashed): data flows this way
- (line=dotted): embedded at build time
- (line=badge, target=inter): reads the bundled font
