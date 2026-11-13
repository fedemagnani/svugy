---
styles:
  service: (color=#3478f6)
  topic: (shape=slab, color=#f0a020)
  store: (shape=cylinder, color=#d5dcea)
---

## Components
- producers: Producers
  - checkout: Checkout (service, caption=places orders)
  - payments: Payments (service, caption=captures cards)
- broker: Kafka (caption=durable log)

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

## Workflows
### checkout: A customer checks out (subtitle="two services, two topics")
One order produces two events, each on its own topic. Neither producer knows who reads them.

- producers (layout=column)
- broker (layout=column)

#### place: Checkout publishes the order
[Checkout](#checkout) appends `order.placed` to [orders](#orders) and answers the customer at once.
- checkout -> orders: publishes (traveler=document)

#### capture: Payments publishes the capture
Once the card is charged, [Payments](#payments) appends `payment.captured` to [payments](#captured).
- payments -> captured: publishes (traveler=document)

### react: Consumers react at their own pace (subtitle="one event, many consumers")
Each consumer group reads [the orders topic](#orders) independently, so a slow [warehouse load](#react/load) never delays [the confirmation email](#react/confirm).

- broker (layout=column)

#### confirm: Notifications emails the customer
[Notifications](#notify) needs only the order to send the confirmation.
- orders -> notify: consumes
- notify -> mail: sends the confirmation (traveler=envelope)

#### ship: Fulfillment ships a paid order
[Fulfillment](#fulfillment) reads both topics and joins them by order id.
- orders -> fulfillment: consumes
- captured -> fulfillment: consumes

  ```description
  Fulfillment waits for both events before it ships.
  ```

#### load: Analytics loads the warehouse hourly
[Analytics](#analytics) buffers both topics and writes one batch per hour.
- orders -> analytics: buffers
- captured -> analytics: buffers
- analytics -> warehouse: loads hourly (traveler=parcel)

### lag: A consumer falls behind (subtitle=the log keeps what it missed)
[Analytics](#analytics) is down for an afternoon. [Kafka](#broker) keeps every event for 7 days, so nothing is lost.

#### pile: Orders keep coming
[Checkout](#checkout) keeps publishing and [Notifications](#notify) keeps emailing; only the analytics offset stops moving.
- checkout -> orders: publishes (traveler=document)
- orders -> notify: consumes
- analytics

#### resume: Analytics resumes from its last offset
On restart it reads from its last committed offset, not from the newest event.
- orders -> analytics: replays the backlog (traveler=document)

#### catch-up: One load closes the gap
The next hourly load carries the whole backlog to [the warehouse](#warehouse).
- analytics -> warehouse: loads the backlog (traveler=parcel)
