# Install svugy

Install the `svugy` CLI, then the svugy skill. Skip a step that is already done.

## 1. CLI

Skip if `svugy --version` prints a version. Otherwise run the command for the user's OS:

| OS | Command |
|---|---|
| macOS, Linux | `curl -fsSL https://svugy.dev/install.sh \| sh` |
| Windows (PowerShell) | `irm https://svugy.dev/install.ps1 \| iex` |
| any OS with `cargo` on `PATH` | `cargo install svugy-cli` |

The installers put `svugy` in `~/.cargo/bin` (`%USERPROFILE%\.cargo\bin` on Windows) and add it to `PATH` for new shells only. In the current shell, call it by that full path.

Check: `svugy --version` prints a version.

## 2. Skill

- Claude Code: `svugy skill install` writes the skill to `.claude/skills/svugy/` in the current project. Add `--user` only if the user asks for every project: it writes to `~/.claude/skills/svugy/` instead.
- Any other agent: `npx skills add fedemagnani/svugy` (needs Node.js).

The installed skill is a copy: after updating svugy, rerun the same command. Agents load skills at session start, so tell the user a new session may be needed to pick it up.
