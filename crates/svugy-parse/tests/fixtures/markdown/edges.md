<!-- Chains, labels, repeated edges, suffix references, a reference spelling a whole path, per-edge properties and descriptions, right under the item line and after a blank line, in backtick and tilde fences; one label holds `:` and `->`. Edges listed again by `[n]` in a later step, one listed by `[n]` before its pair is declared, a bare item, and a second workflow numbering its own edge between a pair the first one also joins. Edge links without `[n]` get it inside a workflow and keep their spelling in a component description; one links to a later parallel edge with `[1]`, one to a step. -->

## Components
- client
  ```description
  Probes [the API](#client->api) in every workflow that shows it.
  ```
- api
- orders
  - api
  - store (shape=cylinder)
- billing
  - api
  - store (shape=cylinder)

## Workflows
### checkout: A client checks out
[The client](#client) pays [billing](#billing.api) through [the charge](#orders.api->billing.api[0]), recapped in [the last step](#checkout/recap).

#### order: The order is stored
- client -> orders.api -> orders.store
- orders.api -> billing.api[1]
- client -> billing.api: pays
  ```description
  Card payments only; invoices go by post. Refunds come back as [a refund](#orders.api->billing.api[1]).
  ```

#### charge: Billing charges and refunds
- orders.api -> billing.api: charges (traveler=envelope)

  ~~~~description
  One charge per order, retried **3 times** with backoff:
  - after 1 s
  - after 10 s

  Paid by [the client's card](#client->billing.api).
  ~~~~

- orders.api -> billing.api: refunds (line=dashed, color=#5b6170, traveler=parcel)
- billing.api -> billing.store (line=badge)
- orders.store -> billing.store (line=dotted, traveler=document)
- client -> api: health: ok -> degraded

#### recap: The client hears back
[The refund](#orders.api->billing.api[1]) reaches [the client](#client) as [a payment](#client->billing.api) undone.
- client -> billing.api[0]
- orders.api -> billing.api[1]

### probe: A probe checks health
<!-- Prose is stored verbatim, minus its comments: a link reference definition, a setext underline and `*`/`+` bullets stay prose. -->
The probe's [own edge](#client->api) is numbered in this workflow alone, as [the health spec][spec] asks.

Health states
---
* ok
+ degraded

[spec]: https://example.com/health

#### ping: The client pings the API
- client -> api: probes
- billing
