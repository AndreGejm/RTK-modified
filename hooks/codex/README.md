# Codex CLI Hooks

> Part of [`hooks/`](../README.md) - see also [`src/hooks/`](../../src/hooks/README.md) for installation code

## Specifics

- Global prompt guidance lives in `rtk-awareness.md`
- `rtk-mcp-server.mjs` is a tiny stdio MCP shim that exposes RTK command rewriting as a Codex tool
- `rtk-mcp-server.yaml` is a Docker Desktop local-server definition for the same shim
- `rtk-awareness.md` is injected into `AGENTS.md` with an `@RTK.md` reference
- Installed to `~/.codex/` by `rtk init --codex`
