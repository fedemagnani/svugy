---
title: A web app
subtitle: from the browser to the database and back
styles:
  service: (color=#3478f6)
  store: (shape=cylinder, color=#d5dcea)
---

```description
An order goes from [the browser](#browser) through [the API](#api) into [the database](#db). [The worker](#worker) sends [the receipt email](#worker->mail) later.
```

## Components
- browser: Browser (caption=the user's tab)
- cdn: CDN (caption=static files, shape=disc)
- backend: Backend (caption=your servers)

  ```description
  Every browser request lands on [the API](#api). From there:
  - the API answers the browser right away
  - work that can wait, like the receipt, goes through [the queue](#queue) to [the worker](#worker)
  ```

  - api: API (service, caption=HTTP endpoints)
  - queue: Queue (shape=slab, caption=pending jobs)
  - worker: Worker (service, caption=background jobs)
- data: Data (caption=what the app remembers)

  ```description
  [The database](#db) keeps users and orders. [The cache](#cache) keeps the reads the API repeats most.
  ```

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
  ```description
  The API answers [the browser](#browser) *before* the receipt is sent.
  ```
- queue -> worker: hands over
- worker -> mail: sends the receipt (traveler=envelope)

  ```description
  The email provider delivers the receipt to the user's inbox.
  ```

