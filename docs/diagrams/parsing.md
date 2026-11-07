---
title: svugy · parsing
subtitle: every language ends in one admission, one model
styles:
  language: (color=#f0a020)
  shared: (color=#5b6170)
  hub: (color=#14f195)
---

<!-- The parsing layer, drawn by svugy: the languages are peers, the model is the layer under them. -->

## Components
- parse: svugy-parse (caption=text in · text out, color=#2a2418, position=0,0)
  - markdown (language, caption=parse · print, position=18,18)
  - d2 (language, caption=import v0.9.0, position=150,18)
  - json (language, caption=load · save, position=282,18)
  - diagnostic (shared, caption=errors · warnings, position=150,180)
  - prose (shared, caption=lists names, position=282,180)
- model: svugy-model (caption=the hub, color=#17271f, position=0,420)
  - builder: ModelBuilder (hub, caption=the one way in, position=18,18)
  - admission (hub, caption=every check, position=150,18)
  - graph: Model (hub, caption=elements · edges, position=282,18)
- pipeline: svugy (caption=layout · geometry · render, color=#1b2436, position=560,440)

## Edges
- markdown -> builder: ①
- d2 -> builder: ①
- json -> admission: ①
- builder -> admission -> graph
- graph -> markdown: ② (line=dotted)
- graph -> json: ② (line=dotted)
- graph -> pipeline: ③
- markdown -> diagnostic (line=badge)
- d2 -> diagnostic (line=badge)
- json -> diagnostic (line=badge)
- diagnostic -> prose (line=badge)

## Legend: Read it as a story
- ① markdown and d2 lower through the builder; json is admitted as saved
- ② markdown and json write a model back as text
- ③ the pipeline reads the model, never the text
- (line=dotted): a model printed back as text
- (line=badge, target=diagnostic): reports through the one harness
