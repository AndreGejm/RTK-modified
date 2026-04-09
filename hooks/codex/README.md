# Codex CLI Hooks

> Part of [`hooks/`](../README.md) - see also [`src/hooks/`](../../src/hooks/README.md) for installation code

## Specifics

- Global prompt guidance lives in `rtk-awareness.md`
- `rtk-mcp-server.mjs` is a tiny stdio MCP shim that exposes RTK command rewriting as a Codex tool
- `rtk-mcp-server.yaml` is the single-server Docker metadata for the same shim
- `rtk-catalog.yaml` is the full Docker Desktop custom catalog entry to register when you want RTK exposed through `docker mcp`
- `rtk-awareness.md` is injected into `AGENTS.md` with an `@RTK.md` reference
- `rtk init --codex` also installs a PATH-visible host launcher on Windows so Docker-backed rewrites can execute `rtk ...` in new Codex workspace shells
- Installed to `~/.codex/` by `rtk init --codex`

`rtk init --codex` installs the instruction files and host launcher only. Registering the Docker MCP gateway remains a separate step that uses the assets in this folder.
