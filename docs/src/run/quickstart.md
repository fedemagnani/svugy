# Quickstart

```sh
curl -fsSL https://svugy.dev/install.sh | sh
svugy example -o webapp.md
svugy render webapp.md -o webapp.svg --open
```

The installer puts `svugy` in `~/.cargo/bin` and adds it to `PATH`. On Windows run `irm https://svugy.dev/install.ps1 | iex`; with a Rust toolchain, `cargo install svugy-cli`.

To update, rerun the command you installed with: it replaces `svugy` with the latest release. `svugy --version` prints the installed version.

`example` writes a sample diagram of a web app; `--open` opens the written SVG in your default browser.

`svugy view webapp.md` serves the diagram in your default browser and reloads it on every save of `webapp.md`, keeping the focused level.

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
| `3` | a file can't be read, written or opened, or `view` can't listen on its port |

## Let an agent draw

The svugy skill teaches an agent the Markdown format and the write, validate, render loop.

- Any agent: `npx skills add fedemagnani/svugy`.
- Claude Code: `svugy skill install` writes the skill to `.claude/skills/svugy/`; `--user` writes it to `~/.claude/skills/svugy/`. `svugy skill` prints it.
- To have an agent install both svugy and the skill, paste it the [installation prompt](https://svugy.dev/install.md).

The installed skill is a copy: after updating svugy, rerun the command you installed it with so the agent follows the new version's rules.

## Other inputs

- `render`, `validate` and `import` read `.md`, `.d2` and `.json`, chosen by extension.
- `svugy import my.d2 -o my.md` converts to Markdown; `-o my.json` writes the model as JSON.

Next: [Writing diagrams](writing-diagrams.md)
