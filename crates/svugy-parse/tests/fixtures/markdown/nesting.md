---
title: Order desk
subtitle: How an order becomes a trade, from click to ledger.
---

<!-- Frontmatter title and subtitle, a diagram description, nesting by indentation, bare and quoted display names, one ending in parentheses, default labels, and descriptions in fences, one holding a bullet list, one a fenced code block, one followed by children; description links to a component declared later, by a dotted suffix, and one inside fenced code, kept as written. -->

```description
Orders arrive from the **client** and settle in [the store](#store).

- the desk matches them
- the ledger records them
```

## Components
- client
  ```description
  Sends orders over [FIX](https://www.fixtrading.org) to [the API](#api).
  ```
- desk: Order desk

  ```description
  Matches orders *during* market hours.

  > Closed on weekends.
  ```

  - api
  - engine: "Matching engine"
    - book

      ````description
      Price levels, best first:

      ```
      [bid](#bid) | ask
      ```
      ````
- ledger: "Ledger (append-only)"
  - store
