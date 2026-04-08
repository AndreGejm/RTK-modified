# RTK - Rust Token Killer (Codex CLI)

Use RTK as the default shell-command rewrite tool in Codex.

## Default Rule

Before running a shell-style command, call the RTK MCP tool `rtk_rewrite_command`.
If it returns `status: "rewritten"`, run the returned `rewrittenCommand`.
If it returns `status: "passthrough"`, run the original command raw.
If it returns `status: "deny"`, do not force RTK; follow normal Codex safety behavior.

## What This Means

- Prefer RTK first for normal developer orientation and workflow commands.
- Keep raw shell for exact-evidence work where compression would hide needed detail.
- Use `rtk proxy <cmd>` when you want tracking without output compression.

## Typical RTK-Friendly Commands

```bash
git status
git log --oneline -20
cargo test --all
pnpm list
docker ps
kubectl get pods -A
tree
ls
```

## Prefer Raw Commands When Precision Matters

- exact diffs, patches, or code review evidence
- failing test output, compiler errors, and stack traces
- raw `rg`/`grep` evidence used to justify edits
- commands where RTK says `passthrough`

## Meta Commands

```bash
rtk gain
rtk gain --history
rtk gain --evaluation
rtk proxy <cmd>
```
