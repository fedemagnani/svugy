---
view: top
---

<!-- Frontmatter view, nesting by indentation, bare and quoted display names, one ending in parentheses, default labels, and descriptions in fences, one holding a bullet list, one a fenced code block, one followed by children; description links to a component declared later, by a dotted suffix, and one inside fenced code, kept as written. Workflow prose holding a paragraph, a quote and a fenced code block with a `- ` line; step prose and a step without prose. -->

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

## Workflows
### trade: Order desk (subtitle="How an order becomes a trade, from click to ledger.")
Orders arrive from the **client** and settle in [the store](#store).

> The desk matches them; the ledger records them.

```
- not an item, a line of code
```

#### match: The desk matches
- client -> api
- book

#### settle: The ledger records
[The book](#engine.book) hands each trade to [the ledger](#ledger).
- store
