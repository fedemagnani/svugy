# Runtime

The runtime is the JavaScript and CSS embedded in every interactive SVG.

Click a container and the view glides into it. Press Escape, or click empty canvas, to back out a level. Hover or focus anything to get a detail panel. Stubs work as doors: click one and you jump to the other end of the edge. The keyboard works too (Tab, Enter, Space, Escape), and with reduced motion turned on it jumps instead of gliding. You can put several diagrams on one page and they won't interfere.

It never builds a CSS selector from your text and never touches `innerHTML`. With script disabled, as inside an `<img>`, the top-level view still renders completely.

The attributes it reads are written down in the [runtime contract](../reference/runtime-contract.md). The code is `runtime/svugy.js` and `runtime/svugy.css`, and `runtime/fixture.svg` is a hand-written document to test it against.
