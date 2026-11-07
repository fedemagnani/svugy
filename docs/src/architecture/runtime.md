# Runtime

The runtime is the JavaScript and CSS embedded in every interactive SVG.

The diagram opens with only its top-level elements shown, containers closed. Click a closed container to open every container at that depth, which shows one more layer; click an open container's slab and the view glides into it. Press Escape, or click empty canvas, to undo one step: hide the deepest layer, or back out a level. Changing layers crossfades the two while the view keeps what you clicked where it was on screen.

Hover or focus anything to get a detail panel, and everything unrelated to it dims: what stays lit is the thing itself, the edges that touch it, their endpoints and the containers around them.

Stubs work as doors: click one and you jump to the other end of the edge. Once you are inside a container, the elevator in the top right corner shows where you are, either as the chain of levels from the root or as a mini-map of the level around you; click a level or the map to go back up. The keyboard works too (Tab, Enter, Space, Escape), and with reduced motion turned on it jumps instead of gliding. You can put several diagrams on one page and they won't interfere.

It never builds a CSS selector from your text and never touches `innerHTML`. With script disabled, as inside an `<img>`, the top-level layer still renders completely.

The attributes it reads are written down in the [runtime contract](../reference/runtime-contract.md). The code is `crates/svugy/runtime/svugy.js` and `crates/svugy/runtime/svugy.css`, and `crates/svugy/runtime/fixture.svg` is a rendered three-layer document to test it against.
