<!-- Chains, labels, repeated edges, suffix references, a reference spelling a whole path, per-edge properties and descriptions, right under the item line and after a blank line, in backtick and tilde fences; one label holds `:` and `->`; descriptions link to an edge without `[n]` and to a later parallel edge with `[1]`. -->

## Components
- client
- api
- orders
  - api
  - store (shape=cylinder)
- billing
  - api
  - store (shape=cylinder)

## Edges
- client -> orders.api -> orders.store
- client -> billing.api: pays
  ```description
  Card payments only; invoices go by post. Refunds come back as [a refund](#orders.api->billing.api[1]).
  ```
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
