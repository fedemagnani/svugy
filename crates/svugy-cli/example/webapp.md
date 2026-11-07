---
title: A web app
subtitle: from the browser to the database and back
styles:
  service: (color=#3478f6)
  store: (shape=cylinder, color=#d5dcea)
---

## Components
- browser: Browser (caption=the user's tab)
- cdn: CDN (caption=static files, shape=disc)
- backend: Backend (caption=your servers)
  - api: API (service, caption=HTTP endpoints)
  - queue: Queue (shape=slab, caption=pending jobs)
  - worker: Worker (service, caption=background jobs)
- data: Data (caption=what the app remembers)
  - db: Database (store, caption=users · orders)
  - cache: Cache (store, caption=hot reads)
- mail: Email provider (caption=outside your servers, color=transparent, line=dashed)

## Edges
- browser -> cdn: loads the page
- browser -> api: signs in (traveler=key)
- browser -> api: places an order (traveler=envelope)
- api -> cache: looks up
- api -> db: saves the order (traveler=document)
- api -> queue: schedules a receipt (traveler=parcel)
- queue -> worker: hands over
- worker -> mail: sends the receipt (traveler=envelope)

