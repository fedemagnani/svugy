---
title: Order events
subtitle: one checkout, many consumers
styles:
  service: (color=#3478f6)
  topic: (shape=slab, color=#f0a020)
  store: (shape=cylinder, color=#d5dcea)
---

```description
[Checkout](#checkout) publishes one event per order. Each consumer reads [the orders topic](#orders) at its own pace, so a slow [warehouse load](#analytics->warehouse) never delays [the confirmation email](#notify->mail).
```

## Components
- producers: Producers (layout=column)
  - checkout: Checkout (service, caption=places orders)
  - payments: Payments (service, caption=captures cards)
- broker: Kafka (caption=durable log, layout=column)

  ```description
  Topics keep events for 7 days. A consumer that falls behind resumes from its last committed offset.
  ```

  - orders: orders (topic, caption=order.placed)
  - captured: payments (topic, caption=payment.captured)
- consumers: Consumers (caption=one group each)
  - fulfillment: Fulfillment (service, caption=picks · packs · ships)
  - notify: Notifications (service, caption=email · push)
  - analytics: Analytics (service, caption=hourly loader)
- warehouse: Data warehouse (store, caption="every event, forever")
- mail: Email provider (color=transparent, line=dashed, caption=outside your systems)

## Edges
- checkout -> orders: publishes (traveler=document)
- payments -> captured: publishes (traveler=document)
- orders -> fulfillment: consumes
- captured -> fulfillment: consumes

  ```description
  Fulfillment waits for both events before it ships.
  ```

- orders -> notify: consumes
- notify -> mail: sends the confirmation (traveler=envelope)
- orders -> analytics
- captured -> analytics
- analytics -> warehouse: loads hourly (traveler=parcel)
