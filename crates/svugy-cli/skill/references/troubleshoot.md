# Troubleshoot svugy

`svugy validate FILE --format json` prints one JSON object. Each entry of `related`:

| Field | Content |
|---|---|
| `message` | the problem, with any `did you mean` |
| `severity` | `error` or `warning` |
| `code` | `svugy::markdown` for format errors; model checks have their own, e.g. `svugy::dangling_reference` |
| `help` | the `did you mean` of a model check, when there is one |
| `labels` | `{label, line, column, span: {offset, length}}`; line and column from 1, span in bytes |

Exit codes: 0 valid (warnings allowed), 1 errors, 2 usage, 3 file unreadable or unwritable.

## Migrating a v3 file

Messages holding `is v3` name their fix; [syntax.md](syntax.md#migrating-from-v3) maps each construct. Fix them all at once: rename `## Edges` to `## Workflows`, add `### main: TITLE` and `#### main: Every edge` above the edges, move frontmatter `title`/`subtitle` and the old description into that `###` heading and its prose, move every `position`, `layout` and `columns` into the workflow's overrides, delete `## Legend` and say its meaning in step prose. Then split `main` into one workflow per scenario.

## Messages whose fix isn't in the message

| Message | Fix |
|---|---|
| `layout` on a component is v3, for a component whose line has no `layout` | a style it uses holds the key: apply that style in the workflow's overrides instead |
| an edge goes under a step heading `#### ID: TITLE` | add a `#### ID: TITLE` line above the workflow's first edge |
| wrap a description in a ```description block, under a step's last item | prose goes between the `####` heading and the first item; move it up |
| unexpected `:`: expected `.` or `(`, on an override | an override has no label: `- api (caption=REST)`, label set in `## Components` |
| step `w/s` lists edge `(a -> b)[n]`, which names nothing in workflow `w` | `[n]` reuses an edge declared in the same workflow, counting from 0; in another workflow, declare the edge again |
| workflow `w` overrides `d`, which it doesn't show | list `d` in a step, as an edge end or bare, or drop the override |
| element `d` is shown by no workflow, so it is never drawn | add it to the step where it acts, as an edge end or `- d`, or delete it |
| no style `Redis`; a label ending in parentheses needs quotes | `- cache: "Cache (Redis)"` |
| edge `(shop -> shop.api)[0]` … joins an element to itself or to its own ancestor | nesting already shows ownership; drop the edge or target a sibling |
| edge … is a badge, which needs a leaf at both ends | target a leaf inside the container, or drop `line=badge` |
| malformed frontmatter: mapping values are not allowed | quote the YAML value holding `: ` |

