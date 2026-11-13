---
styles:
  service: (color=#3478f6, caption=a service)
  store: (shape=cylinder, color=#d5dcea)
  async: (line=dashed, traveler=envelope)
  row: (layout=row, color=#1b2436)
---

<!-- Styles from the frontmatter: shared by components, edges and overrides, applied first, overridden by key=value entries; a style holding `layout` used in an override. -->

## Components
- api (service)
- worker (service, caption=drains the queue)
- db (store)
- archive (store, service)
- storage
  - hot
  - cold

## Workflows
### jobs: Jobs flow to storage
- storage (row, color=#334566)
- db (service)

#### enqueue: The API enqueues
- api -> worker: enqueue (async)
- worker -> db (async, traveler=document)
- worker -> archive

#### tier: Storage tiers
- hot
- cold
