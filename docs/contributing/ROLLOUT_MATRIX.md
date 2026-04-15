# RTK Rollout Matrix

_Last updated: 2026-04-07_

This document defines which command families are safe for **default automatic rewrite**, which are still **pilot-only**, and which stay **blocked / manual-only**.

## Invariants Required For `Allow`

A family can be marked `Allow` only if all of these are true:

- Nonzero exit preserves raw evidence or provides a trivial raw recovery path.
- Lossy success output is visibly labeled as `[summary view]`.
- Presentation logic never changes the real exit code.
- Recovery hints are available whenever detail is removed.
- The family has an explicit evidence-sensitivity / auto-rewrite policy.
- Regression tests exist for the rollout-critical path.

## Status Meanings

- `Allow` — safe for broad default rewrite.
- `PilotOnly` — useful, but still needs targeted validation before default rewrite.
- `Blocked` — keep raw by default or invoke manually only.
- `ManualOnly` — intentionally not part of the default rewrite lane.

## Default Rewrite Allowlist

| Family | Command surface | Implementation path | Success mode | Failure mode | Exit truth | Recovery | Tests | Status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Git evidence | `git diff`, `git show`, `git log` | `src/cmds/git/git.rs` | Raw default, compact only on explicit opt-in | Raw | Preserved | Direct raw output | `discover::registry`, git command tests | `Allow` |
| Search evidence | `grep`, `rg` | `src/cmds/system/grep_cmd.rs` | Raw default unless explicit compacting | Raw | Preserved | Tee-backed when summarized | grep unit tests + rewrite tests | `Allow` |
| File reads | `cat`, `head`, `tail` via `rtk read` | `src/cmds/system/read.rs` | Raw default, deterministic summaries when requested | Raw | Preserved | Tee-backed when summarized | read tests + rewrite tests | `Allow` |
| Low-risk inventory | `ls`, `tree`, `wc` | `src/cmds/system/*.rs` | Compact summary | Raw / passthrough | Preserved | Not usually needed | unit + rewrite tests | `Allow` |
| Rust diagnostics | `cargo build`, `cargo check`, `cargo clippy`, `cargo test` | `src/cmds/rust/*` | Labeled summary on lossy success | Raw | Preserved | Tee-backed on lossy output | rust runner + rewrite tests | `Allow` |
| Python diagnostics | `pytest`, `mypy`, `ruff` | `src/cmds/python/*` | Labeled summary on lossy success | Raw | Preserved | Tee-backed on lossy output | unit + rewrite tests | `Allow` |
| Go diagnostics | `go build`, `go test`, `go vet`, `golangci-lint` | `src/cmds/go/*` | Labeled summary on lossy success | Raw | Preserved | Tee-backed on lossy output | go unit + rewrite tests | `Allow` |
| .NET diagnostics | `dotnet build`, `dotnet test`, `dotnet restore`, `dotnet format` | `src/cmds/dotnet/*` | Labeled summary on lossy success | Raw | Preserved | Tee-backed on lossy output | extensive .NET unit tests | `Allow` |
| JS/TS diagnostics | `tsc`, `lint`, `next build`, `vitest`, `playwright` | `src/cmds/js/*` | Labeled summary on lossy success | Raw | Preserved | Tee-backed on lossy output | js unit + rewrite tests | `Allow` |
| Ruby diagnostics | `rspec`, `rubocop`, `rake test` | `src/cmds/ruby/*` | Labeled summary on lossy success | Raw | Preserved | Tee-backed on lossy output | ruby unit + rewrite tests | `Allow` |
| Python package inventory | `pip list`, `pip outdated`, `uv pip list`, `uv pip outdated` | `src/cmds/python/pip_cmd.rs` | Labeled summary on lossy success | Raw | Preserved | Tee-backed on lossy output | `core::policy`, `discover::registry`, pip filter tests | `Allow` |
| pnpm package inventory | `pnpm list`, `pnpm ls`, `pnpm outdated` | `src/cmds/js/pnpm_cmd.rs` | Labeled summary on lossy success | Raw | Preserved | Tee-backed on lossy output | `core::policy`, `discover::registry`, pnpm parser tests | `Allow` |
| Container inventory | `docker ps`, `docker images`, `docker compose ps` | `src/cmds/cloud/container.rs` | Labeled summary on lossy success | Raw | Preserved | Tee-backed on lossy output | container formatter tests + `core::policy` + `discover::registry` | `Allow` |
| Kubernetes inventory | `kubectl get pods`, `kubectl get services` | `src/cmds/cloud/container.rs` | Labeled summary on lossy success | Raw | Preserved | Tee-backed on lossy output | `core::policy` + `discover::registry` | `Allow` |

## Pilot-Only Families

| Family | Command surface | Why not default yet | Current behavior | Status |
| --- | --- | --- | --- | --- |
| Git workflow ops | `git status`, `git add`, `git commit`, `git push`, `git pull`, `git branch`, `git fetch`, `git stash`, `git worktree` | Custom success compaction still needs explicit matrix-level validation for day-to-day workflow evidence | Safer than before, but not promoted to default rewrite | `PilotOnly` |
| GitHub CLI | `gh ...` | Mixed read/write semantics and API-shaped output still need focused validation | Policy keeps out of default rewrite | `PilotOnly` |
| Container logs / build | `docker logs`, `docker compose logs`, `docker compose build` | Operational and build evidence is still more nuanced than inventory summaries | Shared runner is safer, but still kept out of default rewrite | `PilotOnly` |
| Kubernetes evidence-heavy ops | `kubectl logs`, other `kubectl get` surfaces | Operational evidence can still be nuanced; needs command-by-command audit | Kept out of default rewrite | `PilotOnly` |
| Prisma | `prisma generate`, `prisma migrate`, `prisma db push` | Shared contract is in place, but schema / migration semantics still deserve pilot validation | Custom path now uses shared output contract | `PilotOnly` |
| npm / prettier | `npm run/exec`, `prettier` | Not yet explicitly classified to the same depth as other diagnostics | No default rewrite | `PilotOnly` |

## Blocked / Raw-Default Families

| Family | Command surface | Why blocked | Status |
| --- | --- | --- | --- |
| Package writes | `pip install`, `pip uninstall`, `pip show`, `uv pip install`, `pnpm install` | Write operations should not enter default rewrite until separately validated | `Blocked` |
| Cloud / HTTP fetch | `curl`, `wget`, `aws ...` | Custom paths are safer now, but still not promoted to automatic broad rewrite | `Blocked` |
| Graphite / stack helpers | `gt ...` | Custom git-adjacent workflow output still needs explicit rollout evidence | `Blocked` |
| Heuristic summarizer | `summary` | Too heuristic for default automatic rewrite | `Blocked` |
| Unknown families | anything not explicitly categorized | Default policy bias is raw | `Blocked` |

## Manual-Only Utilities

These commands are useful, but they are not part of the default rewrite path and should stay explicit:

- `json`
- `deps`
- `log`
- `find`
- `env`
- `format`
- `local-llm`

## Rollout Rule

For broad default rollout, only the `Allow` table is eligible for automatic rewrite.

Everything else must stay:

- blocked from automatic rewrite,
- explicit / manual only, or
- gated behind a focused pilot.

## Next Promotion Checklist

Before promoting any `PilotOnly` or `Blocked` family to `Allow`, verify:

1. raw-on-failure behavior with real fixtures,
2. labeled summary behavior on lossy success,
3. raw recovery hint presence when detail is removed,
4. exit-code preservation,
5. rewrite-policy test coverage,
6. at least one family-specific regression test covering the risky path.
