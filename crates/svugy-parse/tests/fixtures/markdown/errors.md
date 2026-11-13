---
# expect: frontmatter `title` is v3: write it in the first workflow's heading, `### main: TITLE (subtitle=SUBTITLE)`
title: every error at once
# expect: frontmatter `subtitle` is v3: write it in the first workflow's heading, `### main: TITLE (subtitle=SUBTITLE)`
subtitle: in one pass
# expect: unknown frontmatter key `views`; did you mean `view`?
views: top
styles:
  service: (color=#3478f6)
  pinned: (position=0,40)
---

<!-- Every error below is reported in one pass; each `expect` names the problem on the line after it. Errors the model's checks own (a step without items, an `[n]` naming no edge, a lineage edge) come from them, labelled at the step or edge. -->
<!-- expect: the diagram's title goes in the first workflow's heading: `### ID: TITLE` -->
# Every error at once
<!-- expect: a ```description block before `## Components` is v3: write it as the first workflow's prose, unfenced -->
```description
What the whole diagram shows.
```
<!-- expect: nothing but the frontmatter goes before `## Components`: write prose under a workflow or step heading -->
- a list before the first section

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
<!-- expect: `position` on a component is v3: set it in each workflow's overrides -->
- dock (position=0,40)
<!-- expect: `columns` on a component is v3: set it in each workflow's overrides -->
- reports (columns=3)
<!-- expect: `layout` on a component is v3: set it in each workflow's overrides -->
- shelf (layout=row)
  - book
<!-- expect: `position` on a component is v3: set it in each workflow's overrides -->
- pin (pinned)
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
<!-- expect: no style `SMTP`; a label ending in parentheses needs quotes -->
- smtp: Mail (SMTP)
<!-- expect: `## Components` holds a `- ` bullet list only -->
* starred
<!-- expect: `traveler` does not apply to a component -->
- relay (traveler=dot)
<!-- expect: the description of element `gateway` links to `#valt`, which names no element `#PATH`, edge `#FROM->TO[n]` or step `#WORKFLOW/STEP` -->
- gateway
  <!-- expect: wrap a description in a ```description block -->
  Routes every request.
  <!-- expect: the only code block allowed here is ```description -->
  ```yaml
  routes: 3
  ```
  ```description
  The edge proxy reads [the vault](#valt).
  ```
  <!-- expect: a second ```description block; merge it into the first -->
  ```description
  Also the TLS terminator.
  ```
  - edge
  <!-- expect: a description goes between its item's line and the items nested under it -->
  ```description
  Written after the nested items.
  ```
<!-- expect: a step heading `#### ID: TITLE` goes under a workflow heading `### ID: TITLE` -->
#### stray: A step outside any workflow

<!-- expect: `## Edges` is v3: write each edge in a step: `### main: TITLE`, then `#### main: TITLE` -->
## Edges
- orders -> billing

<!-- expect: `## Legend` is v3 and removed: say what each entry meant in step prose or edge descriptions -->
## Legend
- dashed means async

<!-- expect: `## Legend` is v3 and removed: say what each entry meant in step prose or edge descriptions -->
## Legend: Read it as a story
- ① your text becomes a model

<!-- expect: unknown section `## Components: more`: expected `## Components` or `## Workflows` -->
## Components: more
- skipped

## Workflows
<!-- expect: `## Workflows` holds workflows, each opened by `### ID: TITLE` -->
Text before the first workflow.

<!-- expect: unexpected end of line: expected `:` -->
### broken
#### vault: The vault
- vault

<!-- expect: a workflow heading takes only `subtitle`, not `subtitel`; did you mean `subtitle`? -->
### keys: Unknown keys (subtitel=typo)
#### vault: The vault
- vault

<!-- expect: a workflow heading takes no style: its only property is `subtitle`; a label ending in parentheses needs quotes -->
### styled: A styled heading (service)
#### vault: The vault
- vault

<!-- expect: unexpected `"q"`: expected a name -->
### listed: A real properties list (subtitle=sub, "q")
<!-- expect: a step heading takes no properties -->
#### vault: The vault (x=1)
- vault

<!-- expect: workflow id `1w` is not an id: ASCII letters, digits, `_` and single inner `-`, starting with a letter or `_` -->
### 1w: A broken ID keeps its body read
Some prose
---
* not an item
<!-- expect: step id `2s` of workflow `1w` is not an id: ASCII letters, digits, `_` and single inner `-`, starting with a letter or `_` -->
#### 2s: Its items are still read
<!-- expect: unexpected end of line: expected a name -->
- vault ->
<!-- expect: no component `nope` -->
- nope
<!-- expect: no style `SMTP`; a label ending in parentheses needs quotes -->
- vault -> worker: Mail (SMTP)

### checks: Overrides, items and the model's checks
<!-- expect: workflow and step prose is unfenced: drop the ```description fence -->
```description
The v3 habit of fencing prose.
```
- orders (layout=row)
<!-- expect: an edge goes under a step heading `#### ID: TITLE`: before the first step, an item overrides a component, `REF (PROPERTIES)` -->
- orders -> billing
- billing (color=red)
  <!-- expect: an override takes no description: describe the component under `## Components` -->
  ```description
  Overrides change drawing only.
  ```
<!-- expect: unexpected end of line: expected `.` or `(` -->
- vault
- worker (color=red)
  <!-- expect: only `## Components` items nest -->
  - nested
<!-- expect: text after the list: prose goes between the heading and the first item -->
Text after the overrides.

#### shapes: Item shapes that take no extras
<!-- expect: a bare item takes no label; write it on an edge declaration `REF -> REF` -->
- orders: a label
<!-- expect: a bare item takes no properties list; write it on an edge declaration `REF -> REF` -->
- billing (color=red)
- worker
  <!-- expect: a bare item takes no description; describe the component under `## Components` -->
  ```description
  Bare.
  ```
<!-- expect: `[n]` lists an edge: `REF -> REF[n]` -->
- vault[0]
<!-- expect: a chain takes no label; write its hops on separate lines -->
- orders -> billing -> audit: flows
<!-- expect: a chain takes no `[n]`; write its hops on separate lines -->
- audit -> reports -> search[0]
- vault -> worker -> audit
  <!-- expect: a chain takes no description; write its hops on separate lines -->
  ```description
  Nightly.
  ```
<!-- expect: text after the list: prose goes between the heading and the first item -->
Text after the items.

#### refs: References, properties and listed edges
<!-- expect: `api` matches `orders.api` and `billing.api`; add a parent to pick one -->
- audit -> api
<!-- expect: no component `ordres`; did you mean `orders`? -->
- ordres -> billing
<!-- expect: `shape` does not apply to an edge -->
- audit -> search (shape=cylinder)
<!-- expect: `[n]` is at most 4294967295 -->
- orders -> billing[99999999999]
<!-- expect: unexpected `x`: expected a whole number -->
- orders -> billing[x]
<!-- expect: `[n]` takes a whole number -->
- orders -> billing[+1]
<!-- expect: an `[n]` item takes no label; write it where the edge is declared -->
- billing -> audit[0]: again
- vault -> worker[0]
  <!-- expect: an `[n]` item takes no description; write it where the edge is declared -->
  ```description
  Listed again.
  ```

<!-- expect: the description of step `checks/several` links to `#audit->reports`, which names several edges in workflow `checks`; add `[n]` to pick one -->
#### several: Two edges between one pair
Which of [the reports](#audit->reports) does it mean? <!-- a comment hides [this link](#nowhere) -->
- audit -> reports: again

<!-- expect: step `checks/empty` lists no edge or component -->
#### empty: No items

#### dangling: An `[n]` naming no edge
<!-- expect: step `checks/dangling` lists edge `(audit -> worker)[0]`, which names nothing in workflow `checks` -->
- audit -> worker[0]

#### lineage: Edges no layer can draw
<!-- expect: edge `(orders -> orders.store)[0]` of workflow `checks` joins an element to itself or to its own ancestor, which no layer can draw -->
- orders -> orders.store
- billing.api
<!-- expect: edge `(audit -> billing)[0]` of workflow `checks` is a badge, which needs a leaf at both ends -->
- audit -> billing (line=badge)

### shown: Every component, so none is left undrawn
#### all: Every component
- orders.api->billing.api
- ledger
- dock
- reports
- shelf.book
- pin
- search
<!-- expect: an ID cannot end with `-` or hold `--` -->
- cache-
- queue
- mailer
- notifier
- smtp
- relay
- gateway.edge

<!-- expect: workflow `tail` has no step, so it shows nothing -->
### tail: A fence never closed
<!-- expect: a code block in prose needs a closing fence as long as its opening one -->
```
#### swallowed: Read as code
- vault
