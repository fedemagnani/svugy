# Writing diagrams

Every input becomes the same model, so a diagram renders to the same SVG whichever language you write it in.

| Input | Use it for |
|---|---|
| [Markdown](markdown.md) (`.md`) | writing diagrams; it can say everything svugy draws |
| [d2](d2.md) (`.d2`) | existing d2 files: render them as they are, or import them to Markdown |
| [Rust](rust.md) | programs that build diagrams |
| JSON (`.json`) | tools that generate diagrams; `svugy import FILE -o model.json` writes one |

- Run `svugy validate` after each edit: a misspelt key, style or reference comes back with the name you probably meant.
- Leave `position` out at first. The automatic layout puts each hop of a chain one step further along, which draws as a diagonal; pin positions only where that shape is wrong.

Next: [Markdown](markdown.md)
