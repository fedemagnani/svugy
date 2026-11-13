---
styles:
  service: (color=#3478f6)
  store: (shape=cylinder, color=#d5dcea)
---

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

## Workflows
### main: A web app takes an order (subtitle=from the browser to the database and back)
A user opens the app, signs in and buys. [The API](#api) answers every request at once; [the receipt](#main/receipt) is sent later.

#### load: The browser loads the page
[The CDN](#cdn) serves the HTML, scripts and images; no request reaches your servers yet.
- browser -> cdn: loads the page

#### sign-in: The user signs in
[The API](#api) checks the password and stores the session in [the cache](#cache), so later requests skip the database.
- browser -> api: signs in (traveler=key)
- api -> cache: stores the session

#### order: The user places an order
[The API](#api) reads [the session](#api->cache[1]) back and saves the order in [the database](#db).
- browser -> api: places an order (traveler=envelope)
- api -> cache: reads the session
- api -> db: saves the order (traveler=document)

#### receipt: The worker sends the receipt
[The API](#api) only queues the receipt; [the worker](#worker) picks it up and hands it to [the email provider](#mail).
- api -> queue: schedules a receipt (traveler=parcel)
  ```description
  The API answers [the browser](#browser) *before* the receipt is sent.
  ```
- queue -> worker: hands over
- worker -> mail: sends the receipt (traveler=envelope)

  ```description
  The email provider delivers the receipt to the user's inbox.
  ```
