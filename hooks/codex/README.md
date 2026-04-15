# Codex CLI Hooks

> Part of [`hooks/`](../README.md) - see also [`src/hooks/`](../../src/hooks/README.md) for installation code

## Specifics

- Prompt-level guidance via awareness document - no programmatic hook
- `rtk-awareness.md` is injected into `AGENTS.md` with an `@RTK.md` reference
- The Codex guidance is intentionally conservative: use `rtk` for summary-safe orientation commands, and prefer raw shell commands for evidence-sensitive output
- Installed to `$CODEX_HOME` when set, otherwise `~/.codex/`, by `rtk init --codex`
