<!-- Legend free lines and key rows for dashed, dotted and badge edges. -->

## Components
- app
- font (shape=cylinder)
- db (shape=cylinder)
- assets

## Edges
- app -> db (line=dashed)
- app -> assets (line=dotted)
- app -> font: "reads" (line=badge)

## Legend: How to read it
- arrows point where data goes
- (line=dashed): writes to storage
- (line=dotted): bundled at build time
- (line=badge, target=font): reads the bundled font
