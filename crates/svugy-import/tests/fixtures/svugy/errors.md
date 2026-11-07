---
title: every error at once
# expect: unknown frontmatter key `subtitel`; did you mean `subtitle`?
subtitel: typo
styles:
  service: (color=#3478f6)
---

<!-- Every error below is reported in one pass; each `expect` names the problem on the line after it. -->

## Components
- orders
  - api (shape=cylinder)
  <!-- expect: unknown key `colour`; did you mean `color`? -->
  - store (colour=#fff)
- billing
  - api
<!-- expect: `ledger` is indented but not nested: the comment above ends its parent's list; indent the comment like the item -->
  - ledger
<!-- expect: `pyramid` is not a shape: expected `box`, `slab`, `cylinder` or `disc` -->
- vault (shape=pyramid)
<!-- expect: no style `servise`; did you mean `service`? -->
- worker (servise)
<!-- expect: `position` takes two numbers, `X,Y` -->
- audit (position=12)
<!-- expect: `columns` needs `layout=grid` -->
- reports (columns=3)
<!-- expect: `caption` appears twice -->
- search (caption="find", caption="lookup")

## Edges
<!-- expect: `api` matches `orders.api` and `billing.api`; add a parent to pick one -->
- audit -> api
<!-- expect: no component `ordres`; did you mean `orders`? -->
- ordres -> billing
<!-- expect: a chain takes no label; write its hops on separate lines -->
- orders -> billing -> audit: "flows"
<!-- expect: an edge cannot join `orders` and its descendant `orders.store` -->
- orders -> orders.store
<!-- expect: a badge edge needs a leaf at both ends; `billing` has children -->
- audit -> billing (line=badge)
<!-- expect: `shape` does not apply to an edge -->
- audit -> search (shape=cylinder)
