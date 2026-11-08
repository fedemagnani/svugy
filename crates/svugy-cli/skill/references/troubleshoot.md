# Troubleshoot svugy

`svugy validate FILE --format json` prints one JSON object. Each entry of `related`:

| Field | Content |
|---|---|
| `message` | the problem, with any `did you mean` |
| `severity` | `error` or `warning` |
| `code` | `svugy::markdown` for format errors; model checks have their own, e.g. `svugy::ignored_container_shape` |
| `labels` | `{label, line, column, span: {offset, length}}`; line and column from 1, span in bytes |

Exit codes: 0 valid (warnings allowed), 1 errors, 2 usage, 3 file unreadable or unwritable.

Messages whose fix isn't in the message:

| Message | Fix |
|---|---|
| `X` is indented but not nested: the comment above ends its parent's list | indent the comment like the item after it |
| a label ending in parentheses needs quotes | `- cache: "Cache (Redis)"` |
| no style `Redis` (after a label) | same: quote the label |
| a chain takes no label | split `a -> b -> c: x` into `a -> b: x` and `b -> c` |
| an edge cannot join `orders` and its descendant | nesting already shows ownership; drop the edge or target a sibling |
| a badge edge needs a leaf at both ends | target a leaf inside the container, or drop `line=badge` |
| malformed frontmatter: mapping values are not allowed | quote the YAML value holding `: ` |
