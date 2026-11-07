# Writing diagrams

Every input becomes the same model, so the same diagram renders to the same SVG whichever you write.

| Input | Use it for |
|---|---|
| [Markdown](markdown.md) (`.md`) | writing diagrams by hand; says everything svugy draws |
| [d2](d2.md) (`.d2`) | existing d2 v0.9.0 files: render them as they are, or `svugy import` them to Markdown |
| [Rust](rust.md) | programs that build diagrams, through the `svugy` crate |
| JSON (`.json`) | tools that generate diagrams; `svugy import FILE -o model.json` writes one from any input |

- Run `svugy validate` after each edit: a misspelt key, style or reference comes back with the name you probably meant.
- Leave `position` out at first. The automatic layout places each hop of a chain one rank further along the plan's x axis, which draws as a long diagonal; pin `position` only where that shape is wrong.

Next: [Markdown](markdown.md)
