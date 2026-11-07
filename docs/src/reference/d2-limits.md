# d2 limits

*Against d2 v0.9.0.*

What d2 can't express, and what svugy does instead.

d2 gives a shape one label, and its `tooltip` is hover text, not a second line, so captions go in `vars.svugy.caption`. d2's shapes are 2D outlines and an unknown `shape:` is a compile error, which means a slab, or an explicit box, has no d2 name. svugy reads `vars.svugy.shape` (`box`, `slab`, `cylinder` or `disc`) and lets it win over d2's `shape:`, which still maps `cylinder`, and `circle` or `oval` to disc.

d2 has no third axis. Its own `height:` is the 2D shape's size in pixels, so svugy uses `vars.svugy.height` in plan units, honoured by every leaf primitive; containers stay slabs and warn, and d2's `height:` still warns. Depth and elevation aren't in the model yet; when they arrive they'll go in `vars.svugy` too.

All of this lives in `vars` because unknown `style.*` keys are a compile error in d2, and a dotted namespace like `iso.depth` becomes child shapes. A shape's own `vars` map is the only place d2 accepts extra attributes.

d2's root `label:` is the board's title; d2 puts it on PDF pages and never draws it in its SVG. svugy reads it as the diagram title and draws it top-left. d2 has no subtitle, so that's `vars.svugy.subtitle` on the root. A root `vars` map used to produce a warning; now it carries this.

d2 v0.9.0 does have a legend, `vars.d2-legend`, but it's a hand-drawn graph of sample shapes and edges, not lines of text or a key read off the diagram. svugy's legend is `vars.svugy.legend { title; entries: [ … ]; dashes; badges }` on the root, and its key rows are generated from what the diagram actually draws. `d2-legend` still warns like any other variable.

d2's `top` and `left` only accept non-negative integers, and only the TALA engine supports them; dagre and elk reject them. svugy pins with `vars.svugy.position { x; y }`, which takes any decimal. d2's `direction:` is a 2D rank flow, and only `right` matches svugy's layered placement, so `right` maps to layered and the others warn. d2's `grid-rows` and `grid-columns` match svugy's grid and row hints, but they still warn for now; they'll become a plain mapping once a fixture needs them.

d2 rejects a dotted `a.vars.x` ("vars" must be the last part of a key), so every carrier is written `a.vars: { svugy: { … } }` or as a `vars` map inside `a`'s own map. d2 merges `vars` across statements, but svugy wants a position's `x` and `y` in one map and rejects half a position. d2 compiles `vars` away without reading them, so a typo there passes d2 silently; svugy reports it with its location.

d2 compiles arrays and nested maps in any field, `vars` included. svugy reads one list, `legend.entries`, plus nested maps, and a map or array inside that list is an error. A `vars` key can be any d2 key, so `5:` and `pipeline.inter:` are both legal. An unquoted dotted key nests and a quoted one stays a single segment, so svugy spells carrier names as model paths: a `badges` key reads exactly like the target's d2 path. d2 classes would remove repeated fills, but classes are outside the v1 subset, so each shape repeats its `style.fill`.

d2 rejects `vars` in an edge map ("does not accept composite"), but svugy reads one there anyway, because an edge's traveler has nowhere else to go. `a -> b: { vars: { svugy: { traveler: envelope } } }` picks the symbol that moves along the edge in the interactive SVG: `dot` (the default), `envelope`, `parcel`, `document` or `key`. An unknown name is an error listing the five, and so is any other key in an edge's carrier. Since d2 itself then refuses the file, set travelers only in files that svugy alone reads. The carrier holds nothing but the traveler. An edge still can't say it should be drawn as a badge, so `vars.svugy.badge: true` goes on the target and badges every edge into it from a leaf. An edge can't carry its own legend row either, so the root legend describes dashes by their `stroke-dash` value and badges by their target's path. Finally, d2 reads `style.stroke-dash` as an integer from 0 to 10 (`1.5` is a d2 error), so dash keys in d2 are integers, while JSON can still use `1.5`.
