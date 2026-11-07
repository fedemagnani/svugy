---
title: Order desk
subtitle: How an order becomes a trade, from click to ledger.
---

<!-- Frontmatter title and subtitle, a diagram description, nesting by indentation, bare and quoted display names, one ending in parentheses, default labels, and descriptions in both forms, one followed by children. -->

Orders arrive from the **client** and settle in the `ledger`.

1. the desk matches them
2. the ledger records them

## Components
- client
  Sends orders over [FIX](https://www.fixtrading.org).
- desk: Order desk

  Matches orders *during* market hours.

  > Closed on weekends.

  - api
  - engine: "Matching engine"
    - book

      Price levels, best first:

      ```
      bid | ask
      ```
- ledger: "Ledger (append-only)"
  - store
