---
title: every error at once
# expect: unknown frontmatter key `subtitel`; did you mean `subtitle`?
subtitel: typo
styles:
  service: (color=#3478f6)
---

<!-- Every error below is reported in one pass; each `expect` names the problem on the line after it. -->
<!-- expect: the title goes in the frontmatter `title` -->
# Every error at once
<!-- expect: text before the first section goes in a comment, `<!-- … -->`, or in the frontmatter `subtitle` -->
Prose before the first section.

## Components
- orders <!-- a trailing comment is ignored -->
  - api (shape=cylinder)
  <!-- expect: unknown key `colour`; did you mean `color`? -->
  - store (colour=#fff)
- billing
  - api
  <!-- expect: `api` is declared twice under `billing` -->
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
- search (caption=find, caption=lookup)
<!-- expect: `search` is declared twice at the top level -->
- search
<!-- expect: an ID cannot end with `-` or hold `--` -->
- cache-
<!-- expect: unexpected end of line: expected a label -->
- queue:
<!-- expect: unexpected `only`: expected `.`, `=`, `,` or `)`; a label ending in parentheses needs quotes -->
- mailer: Mail relay (SMTP only)
<!-- expect: a label is either quoted whole or holds no `"` -->
- notifier: "Notifier" service

<!-- expect: use `## Edges` -->
### Edges
<!-- expect: `api` matches `orders.api` and `billing.api`; add a parent to pick one -->
- audit -> api
<!-- expect: no component `ordres`; did you mean `orders`? -->
- ordres -> billing
<!-- expect: a chain takes no label; write its hops on separate lines -->
- orders -> billing -> audit: flows
<!-- expect: an edge cannot join `orders` and its descendant `orders.store` -->
- orders -> orders.store
<!-- expect: a badge edge needs a leaf at both ends; `billing` has children -->
- audit -> billing (line=badge)
<!-- expect: `shape` does not apply to an edge -->
- audit -> search (shape=cylinder)
<!-- expect: an edge cannot join `audit` to itself -->
- audit -> audit

## Legend
- (optional) parts are grey
