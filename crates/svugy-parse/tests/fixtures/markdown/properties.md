<!-- Every component property, quoted values, bare values with and without spaces, Markdown inline syntax read verbatim; overrides holding position in both spellings, layout and columns, and any component key over the component's own. -->

## Components
- gateway: API gateway (caption="edge, rate limits", color=#9945ff, stroke=navy, line=dashed, link=https://example.com, code=crates/gateway/src/lib.rs#Gateway::route, meta.owner="platform team", meta.tier=1)
- queue (shape=disc, height=12, color=transparent)
- cache (shape=cylinder, height=40, color=#d5dcea80, caption="*not* emphasis, `not` code, snake_case_kept")
- services (caption=four of them)
  - a
  - b (link=queue)
  - c
  - d

## Workflows
### deploy: Everything deployed (subtitle="pinned, gridded, recoloured")
- gateway (position=0,40)
- queue (position=260;0, caption=drained nightly, meta.tier=2)
- services (layout=grid, columns=2, color=#14f195, link=gateway)
- cache (shape=box, height=20, line=dotted, code=crates/cache/src/lib.rs)

#### all: Every component
- gateway
- queue
- cache
- a
- b
- c
- d
