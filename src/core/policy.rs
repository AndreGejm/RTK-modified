//! Central evidence-safety policy for command output handling.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceSensitivity {
    /// Orientation-style commands where compact summaries are acceptable by default.
    SummarySafe,
    /// Commands that may summarize on success, but must fail open to raw evidence.
    EvidenceSensitive,
    /// Commands whose output is evidence and should stay raw unless the user explicitly opts in.
    RawDefault,
}

impl EvidenceSensitivity {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn allows_lossy_by_default(self) -> bool {
        matches!(self, Self::SummarySafe)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn must_fail_open_to_raw(self) -> bool {
        !matches!(self, Self::SummarySafe)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoRewriteStatus {
    /// Safe enough for the default global rewrite path.
    Allow,
    /// Potentially useful, but not yet uniform enough for the default path.
    PilotOnly,
    /// Must stay raw or be invoked manually until hardened.
    Blocked,
}

impl AutoRewriteStatus {
    pub fn allows_by_default(self) -> bool {
        matches!(self, Self::Allow)
    }
}

pub fn classify_command(command: &str) -> EvidenceSensitivity {
    match command {
        "ls"
        | "tree"
        | "wc"
        | "docker ps"
        | "docker images"
        | "docker compose ps"
        | "kubectl get pods"
        | "kubectl get services"
        | "pnpm list"
        | "npm list"
        | "pip list"
        | "pnpm outdated"
        | "deps"
        | "env" => EvidenceSensitivity::SummarySafe,
        "cargo build" | "cargo check" | "cargo clippy" | "cargo test" | "cargo nextest"
        | "pytest" | "mypy" | "ruff" | "tsc" | "lint" | "golangci-lint" | "dotnet build"
        | "dotnet test" | "dotnet format" | "dotnet restore" | "go build" | "go test"
        | "go vet" | "next build" | "vitest" | "playwright" | "rspec" | "rubocop" | "rake"
        | "prisma generate" | "prisma migrate" | "prisma db push" | "pnpm install" | "err"
        | "test" => EvidenceSensitivity::EvidenceSensitive,
        "grep" | "rg" | "search" | "read" | "cat" | "git diff" | "git show" | "git log" => {
            EvidenceSensitivity::RawDefault
        }
        _ => EvidenceSensitivity::RawDefault,
    }
}

pub fn classify_auto_rewrite(rtk_equivalent: &str, command: &str) -> AutoRewriteStatus {
    match rtk_equivalent {
        "rtk git" => {
            if matches_any_command_prefix(command, &["git diff", "git show", "git log"]) {
                AutoRewriteStatus::Allow
            } else {
                AutoRewriteStatus::PilotOnly
            }
        }
        "rtk cargo" => {
            if matches_any_command_prefix(
                command,
                &["cargo build", "cargo check", "cargo clippy", "cargo test"],
            ) {
                AutoRewriteStatus::Allow
            } else {
                AutoRewriteStatus::Blocked
            }
        }
        "rtk grep" => {
            if matches_any_command_prefix(command, &["grep", "rg"]) {
                AutoRewriteStatus::Allow
            } else {
                AutoRewriteStatus::Blocked
            }
        }
        "rtk read" => {
            if matches_any_command_prefix(command, &["cat", "head", "tail"]) {
                AutoRewriteStatus::Allow
            } else {
                AutoRewriteStatus::Blocked
            }
        }
        "rtk ls" => {
            if matches_any_command_prefix(command, &["ls"]) {
                AutoRewriteStatus::Allow
            } else {
                AutoRewriteStatus::Blocked
            }
        }
        "rtk tree" => {
            if matches_any_command_prefix(command, &["tree"]) {
                AutoRewriteStatus::Allow
            } else {
                AutoRewriteStatus::Blocked
            }
        }
        "rtk wc" => {
            if matches_any_command_prefix(command, &["wc"]) {
                AutoRewriteStatus::Allow
            } else {
                AutoRewriteStatus::Blocked
            }
        }
        "rtk tsc" => {
            if matches_any_command_prefix(command, &["tsc", "npx tsc", "pnpm tsc"]) {
                AutoRewriteStatus::Allow
            } else {
                AutoRewriteStatus::Blocked
            }
        }
        "rtk lint" => {
            if matches_any_command_prefix(
                command,
                &[
                    "eslint",
                    "biome",
                    "lint",
                    "npx eslint",
                    "npx biome",
                    "pnpm lint",
                ],
            ) {
                AutoRewriteStatus::Allow
            } else {
                AutoRewriteStatus::Blocked
            }
        }
        "rtk next" => {
            if matches_any_command_prefix(
                command,
                &["next build", "npx next build", "pnpm next build"],
            ) {
                AutoRewriteStatus::Allow
            } else {
                AutoRewriteStatus::Blocked
            }
        }
        "rtk vitest" => {
            if matches_any_command_prefix(command, &["vitest", "jest", "npx vitest", "pnpm vitest"])
            {
                AutoRewriteStatus::Allow
            } else {
                AutoRewriteStatus::Blocked
            }
        }
        "rtk playwright" => {
            if matches_any_command_prefix(
                command,
                &["playwright", "npx playwright", "pnpm playwright"],
            ) {
                AutoRewriteStatus::Allow
            } else {
                AutoRewriteStatus::Blocked
            }
        }
        "rtk mypy" => {
            if matches_any_command_prefix(command, &["mypy", "python -m mypy", "python3 -m mypy"]) {
                AutoRewriteStatus::Allow
            } else {
                AutoRewriteStatus::Blocked
            }
        }
        "rtk pytest" => {
            if matches_any_command_prefix(
                command,
                &["pytest", "python -m pytest", "python3 -m pytest"],
            ) {
                AutoRewriteStatus::Allow
            } else {
                AutoRewriteStatus::Blocked
            }
        }
        "rtk ruff" => {
            if matches_any_command_prefix(command, &["ruff check", "ruff format"]) {
                AutoRewriteStatus::Allow
            } else {
                AutoRewriteStatus::Blocked
            }
        }
        "rtk go" => {
            if matches_any_command_prefix(command, &["go build", "go test", "go vet"]) {
                AutoRewriteStatus::Allow
            } else {
                AutoRewriteStatus::Blocked
            }
        }
        "rtk golangci-lint" | "rtk golangci-lint run" => {
            if matches_any_command_prefix(command, &["golangci-lint"]) {
                AutoRewriteStatus::Allow
            } else {
                AutoRewriteStatus::Blocked
            }
        }
        "rtk dotnet" => {
            if matches_any_command_prefix(
                command,
                &[
                    "dotnet build",
                    "dotnet test",
                    "dotnet format",
                    "dotnet restore",
                ],
            ) {
                AutoRewriteStatus::Allow
            } else {
                AutoRewriteStatus::Blocked
            }
        }
        "rtk rspec" => {
            if matches_any_command_prefix(command, &["rspec", "bundle exec rspec", "bin/rspec"]) {
                AutoRewriteStatus::Allow
            } else {
                AutoRewriteStatus::Blocked
            }
        }
        "rtk rubocop" => {
            if matches_any_command_prefix(command, &["rubocop", "bundle exec rubocop"]) {
                AutoRewriteStatus::Allow
            } else {
                AutoRewriteStatus::Blocked
            }
        }
        "rtk rake" => {
            if matches_any_command_prefix(
                command,
                &[
                    "rake test",
                    "rails test",
                    "bundle exec rake test",
                    "bundle exec rails test",
                    "bin/rails test",
                ],
            ) {
                AutoRewriteStatus::Allow
            } else {
                AutoRewriteStatus::Blocked
            }
        }
        "rtk pip" => {
            if matches_any_command_prefix(
                command,
                &["pip list", "pip outdated", "uv pip list", "uv pip outdated"],
            ) {
                AutoRewriteStatus::Allow
            } else {
                AutoRewriteStatus::Blocked
            }
        }
        "rtk pnpm" => {
            if matches_any_command_prefix(command, &["pnpm list", "pnpm ls", "pnpm outdated"]) {
                AutoRewriteStatus::Allow
            } else {
                AutoRewriteStatus::Blocked
            }
        }
        "rtk docker" => {
            if matches_any_command_prefix(
                command,
                &["docker ps", "docker images", "docker compose ps"],
            ) {
                AutoRewriteStatus::Allow
            } else {
                AutoRewriteStatus::PilotOnly
            }
        }
        "rtk kubectl" => {
            if matches_any_command_prefix(command, &["kubectl get pods", "kubectl get services"]) {
                AutoRewriteStatus::Allow
            } else {
                AutoRewriteStatus::PilotOnly
            }
        }
        "rtk gh" | "rtk prisma" | "rtk npm" | "rtk prettier" => AutoRewriteStatus::PilotOnly,
        _ => AutoRewriteStatus::Blocked,
    }
}

pub fn allows_automatic_rewrite(rtk_equivalent: &str, command: &str) -> bool {
    classify_auto_rewrite(rtk_equivalent, command).allows_by_default()
}

fn matches_any_command_prefix(command: &str, prefixes: &[&str]) -> bool {
    prefixes
        .iter()
        .copied()
        .any(|prefix| matches_command_prefix(command, prefix))
}

fn matches_command_prefix(command: &str, prefix: &str) -> bool {
    let trimmed = command.trim();
    trimmed == prefix
        || (trimmed.len() > prefix.len()
            && trimmed.starts_with(prefix)
            && trimmed.as_bytes()[prefix.len()].is_ascii_whitespace())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_default_commands_require_explicit_summary_request() {
        assert_eq!(classify_command("grep"), EvidenceSensitivity::RawDefault);
        assert_eq!(classify_command("read"), EvidenceSensitivity::RawDefault);
        assert_eq!(
            classify_command("git diff"),
            EvidenceSensitivity::RawDefault
        );
    }

    #[test]
    fn test_evidence_sensitive_commands_fail_open_to_raw() {
        assert_eq!(
            classify_command("pytest"),
            EvidenceSensitivity::EvidenceSensitive
        );
        assert_eq!(
            classify_command("cargo build"),
            EvidenceSensitivity::EvidenceSensitive
        );
        assert_eq!(
            classify_command("playwright"),
            EvidenceSensitivity::EvidenceSensitive
        );
        assert_eq!(
            classify_command("ruff"),
            EvidenceSensitivity::EvidenceSensitive
        );
        assert!(classify_command("dotnet test").must_fail_open_to_raw());
    }

    #[test]
    fn test_summary_safe_commands_allow_lossy_by_default() {
        assert_eq!(classify_command("ls"), EvidenceSensitivity::SummarySafe);
        assert_eq!(
            classify_command("pnpm outdated"),
            EvidenceSensitivity::SummarySafe
        );
        assert_eq!(
            classify_command("docker compose ps"),
            EvidenceSensitivity::SummarySafe
        );
        assert!(classify_command("tree").allows_lossy_by_default());
    }

    #[test]
    fn test_unknown_commands_bias_to_raw_default() {
        assert_eq!(
            classify_command("totally-unknown"),
            EvidenceSensitivity::RawDefault
        );
    }

    #[test]
    fn test_auto_rewrite_allows_hardened_git_and_blocks_pilot_git() {
        assert_eq!(
            classify_auto_rewrite("rtk git", "git diff --cached"),
            AutoRewriteStatus::Allow
        );
        assert_eq!(
            classify_auto_rewrite("rtk git", "git status"),
            AutoRewriteStatus::PilotOnly
        );
    }

    #[test]
    fn test_auto_rewrite_allows_core_hardened_families() {
        assert!(allows_automatic_rewrite("rtk cargo", "cargo test"));
        assert!(allows_automatic_rewrite(
            "rtk pytest",
            "python -m pytest -x tests/"
        ));
        assert!(allows_automatic_rewrite("rtk read", "head -20 src/main.rs"));
        assert!(allows_automatic_rewrite("rtk vitest", "pnpm vitest run"));
        assert!(allows_automatic_rewrite(
            "rtk golangci-lint run",
            "golangci-lint --color never run ./..."
        ));
        assert!(allows_automatic_rewrite(
            "rtk rake",
            "bundle exec rails test"
        ));
    }

    #[test]
    fn test_auto_rewrite_blocks_pilot_and_unhardened_families() {
        assert_eq!(
            classify_auto_rewrite("rtk gh", "gh pr list"),
            AutoRewriteStatus::PilotOnly
        );
        assert_eq!(
            classify_auto_rewrite("rtk prisma", "npx prisma migrate dev"),
            AutoRewriteStatus::PilotOnly
        );
        assert_eq!(
            classify_auto_rewrite("rtk docker", "docker ps"),
            AutoRewriteStatus::Allow
        );
        assert_eq!(
            classify_auto_rewrite("rtk docker", "docker images"),
            AutoRewriteStatus::Allow
        );
        assert_eq!(
            classify_auto_rewrite("rtk docker", "docker compose ps"),
            AutoRewriteStatus::Allow
        );
        assert_eq!(
            classify_auto_rewrite("rtk kubectl", "kubectl get pods -A"),
            AutoRewriteStatus::Allow
        );
        assert_eq!(
            classify_auto_rewrite("rtk kubectl", "kubectl get services -n prod"),
            AutoRewriteStatus::Allow
        );
        assert_eq!(
            classify_auto_rewrite("rtk docker", "docker compose logs web"),
            AutoRewriteStatus::PilotOnly
        );
        assert_eq!(
            classify_auto_rewrite("rtk kubectl", "kubectl logs api-123"),
            AutoRewriteStatus::PilotOnly
        );
    }

    #[test]
    fn test_auto_rewrite_allows_hardened_package_inventory_commands_only() {
        assert_eq!(
            classify_auto_rewrite("rtk pip", "pip list"),
            AutoRewriteStatus::Allow
        );
        assert_eq!(
            classify_auto_rewrite("rtk pip", "pip outdated"),
            AutoRewriteStatus::Allow
        );
        assert_eq!(
            classify_auto_rewrite("rtk pip", "uv pip list"),
            AutoRewriteStatus::Allow
        );
        assert_eq!(
            classify_auto_rewrite("rtk pnpm", "pnpm list --depth=2"),
            AutoRewriteStatus::Allow
        );
        assert_eq!(
            classify_auto_rewrite("rtk pnpm", "pnpm outdated"),
            AutoRewriteStatus::Allow
        );
    }

    #[test]
    fn test_auto_rewrite_keeps_package_write_commands_blocked() {
        assert_eq!(
            classify_auto_rewrite("rtk pip", "pip install requests"),
            AutoRewriteStatus::Blocked
        );
        assert_eq!(
            classify_auto_rewrite("rtk pip", "uv pip install requests"),
            AutoRewriteStatus::Blocked
        );
        assert_eq!(
            classify_auto_rewrite("rtk pnpm", "pnpm install lodash"),
            AutoRewriteStatus::Blocked
        );
    }
}
