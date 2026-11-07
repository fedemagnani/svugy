# Quickstart

```sh
cargo install --git https://github.com/fedemagnani/svugy svugy-cli
svugy example -o webapp.md
svugy render webapp.md -o webapp.svg --open
```

`example` writes a sample diagram of a web app; `--open` opens the written SVG in your default browser.

## Edit and validate

Edit `webapp.md` ([Markdown](markdown.md) has every rule), then run `svugy validate webapp.md` before rendering.

- `validate` prints every error and warning in one run, each with line and column. Errors stop every command; warnings never do.
- `--format json` prints one JSON object on stdout, even for a clean file.
- `render --profile static` writes an SVG with no CSS, script, fonts or animation, text drawn as paths: for `<img>` tags, READMEs and design tools.

| Exit code | Meaning |
|---|---|
| `0` | success, warnings included |
| `1` | the input has errors |
| `2` | usage error, e.g. an unknown flag or an input not ending in `.md`, `.d2` or `.json` |
| `3` | a file can't be read, written or opened |

## Let Claude Code draw

`svugy skill install` writes a skill teaching the Markdown format and the write, validate, render loop to `.claude/skills/svugy/`; `--user` writes it to `~/.claude/skills/svugy/`. `svugy skill` prints it.

## Other inputs

- `render`, `validate` and `import` read `.md`, `.d2` and `.json`, chosen by extension.
- `svugy import my.d2 -o my.md` converts to Markdown; `-o my.json` writes the model as JSON.

Next: [Writing diagrams](writing-diagrams.md)
