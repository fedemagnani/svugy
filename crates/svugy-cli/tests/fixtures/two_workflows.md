<!-- Two workflows over the same components, for the commands that pick one or write each. -->

## Components
- web
- api

## Workflows
### order: An order is placed
#### place: The web app posts it
- web -> api: places order

### refund: An order is refunded
#### reverse: The API refunds it
- api -> web: refunds
