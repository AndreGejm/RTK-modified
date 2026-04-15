# RTK - Rust Token Killer (Codex CLI)

**Usage**: Token-optimized CLI proxy for low-risk shell summaries.

## Rule

Use `rtk` selectively for summary-safe orientation commands.
Prefer raw shell commands whenever exact evidence matters.

### Good candidates for `rtk`

```bash
rtk ls
rtk tree
rtk git status
rtk docker ps
rtk kubectl pods
rtk pnpm list
```

### Prefer raw commands instead

- `git diff`, `git show`, `git log` unless you explicitly want `--compact`
- failing tests, stack traces, compiler errors, and linter diagnostics
- `grep`/`rg` when results are being used as evidence
- code reads used for editing, review, or patch reasoning

## Raw Bypass

```bash
git diff
pytest -q
rtk proxy cargo test
```

Use `rtk proxy <cmd>` when you want tracking without filtering.

## Meta Commands

```bash
rtk gain            # Token savings analytics
rtk gain --history  # Recent command savings history
rtk proxy <cmd>     # Run raw command without filtering
```

## Verification

```bash
rtk --version
rtk gain
which rtk
```
