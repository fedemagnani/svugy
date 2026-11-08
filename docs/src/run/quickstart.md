# Quickstart

```sh
curl -fsSL https://svugy.dev/install.sh | sh   # macOS, Linux
irm https://svugy.dev/install.ps1 | iex        # Windows
cargo install svugy-cli                        # any OS, with Rust
```

Rerun the same command to update.

```sh
svugy example -o webapp.md                     # write a sample diagram
svugy render webapp.md -o webapp.svg --open    # render it and open it
svugy view webapp.md                           # live preview, reloaded on every save
svugy validate webapp.md                       # every error and warning, with line and column
```

- `render --profile static` writes an SVG with no CSS, script or fonts, text drawn as paths: for `<img>`, READMEs and design tools.
- `validate --format json` prints one JSON object, for scripts and agents.
- `render`, `validate` and `import` read `.md`, `.d2` and `.json`, by extension. `svugy import my.d2 -o my.md` converts to Markdown.
- Errors stop every command; warnings never do.

| Exit code | Meaning |
|---|---|
| `0` | success, warnings included |
| `1` | the input has errors |
| `2` | usage error, e.g. an unknown flag or extension |
| `3` | a file can't be read or written, or `view` can't listen on its port |

## Agents

The svugy skill teaches an agent the format and the write, validate, render loop.

- Any agent: `npx skills add fedemagnani/svugy`.
- Claude Code: `svugy skill install`, or `svugy skill install --user` for every project.
- Or paste your agent the [installation prompt](https://svugy.dev/install.md): it installs svugy and the skill.

The skill is a copy: reinstall it after updating svugy.

Next: [Writing diagrams](writing-diagrams.md)
