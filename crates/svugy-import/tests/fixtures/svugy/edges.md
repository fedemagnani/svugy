<!-- Chains, labels, repeated edges, suffix references and per-edge properties. -->

## Components
- client
- orders
  - api
  - store (shape=cylinder)
- billing
  - api
  - store (shape=cylinder)

## Edges
- client -> orders.api -> orders.store
- client -> billing.api: "pays"
- orders.api -> billing.api: "charges" (traveler=envelope)
- orders.api -> billing.api: "refunds" (line=dashed, color=#5b6170, traveler=parcel)
- billing.api -> billing.store (line=badge)
- orders.store -> billing.store (line=dotted, traveler=document)
