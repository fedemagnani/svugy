---
styles:
  service: (color=#3478f6, caption="a service")
  store: (shape=cylinder, color=#d5dcea)
  async: (line=dashed, traveler=envelope)
---

<!-- Styles from the frontmatter: shared by components and edges, applied first, overridden by key=value entries. -->

## Components
- api (service)
- worker (service, caption="drains the queue")
- db (store)
- archive (store, service)

## Edges
- api -> worker: "enqueue" (async)
- worker -> db (async, traveler=document)
- worker -> archive
