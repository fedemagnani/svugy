---
title: svugy · layout
subtitle: Model in, one LevelPlan per container interior out
styles:
  input: (color=#3a3f4b)
  step: (color=#3478f6)
  data: (shape=slab, color=#d5dcea)
  crate: (shape=cylinder, color=#d5dcea)
---

## Components
- model: Model (input, caption=svugy-model)
- font: TextMeasure (input, caption=BundledFont)
- layout: layout (caption=LevelPlans::lay_out)
  - edges: edge_levels (caption=which levels an edge touches)
    - homed: HomedEdges (step, caption=edge → its lowest common ancestor)
    - leaving: LeavingEdges (step, caption=edge → containers it leaves)
  - place: place (caption=bottom-up: children before parents)
    - siblings: SiblingGraph (step, caption=child sizes · joined pairs)
    - layered: Layered (step, caption=default)
    - packed: Packed (step, caption=grid · row · column)
    - pins: Footprints (step, caption=free ones placed around pins)
  - placed: PlacedLevels (data, caption=extent · footprints)
  - route: anchor_homed_routes (step, caption=a port on the facing side)
  - stub: side_stubs (step, caption=a boundary slot per outside endpoint)
  - plans: LevelPlans (data, caption=placed · routes · stubs)
- dagre: svugy-dagre (crate)
- taffy: taffy (crate)
- geometry: geometry (input, caption=projects to screen space)

## Edges
- model -> homed
- model -> leaving
- model -> siblings: children
- font -> siblings: label sizes
- homed -> siblings: joined pairs
- siblings -> layered
- siblings -> packed: layout hint
- layered -> pins
- packed -> pins
- pins -> placed
- placed -> siblings: a container's extent
- layered -> dagre (line=badge)
- packed -> taffy (line=badge)
- placed -> route
- homed -> route
- placed -> stub
- route -> stub: where routes cross
- leaving -> stub
- placed -> plans
- route -> plans
- stub -> plans
- plans -> geometry
