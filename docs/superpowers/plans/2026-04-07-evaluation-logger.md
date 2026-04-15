# Evaluation Logger Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a temporary, toggleable evaluation logger inside RTK's existing tracking database so we can measure total RTK uses, successful RTK handling, recovered/failing RTK paths, and compression success rate during an evaluation period.

**Architecture:** Keep token tracking intact and add a small evaluation layer in the same SQLite database. Record exactly one evaluation event per operational RTK invocation, classify it with a compact outcome enum, and expose the results through `rtk gain --evaluation` so the feature can be enabled during the trial and ignored afterward.

**Tech Stack:** Rust, rusqlite, clap, existing RTK tracking/config/gain modules

---

## File Map

- Modify: `src/core/config.rs`
  Purpose: Add an evaluation toggle that can be enabled or disabled without changing code.

- Modify: `src/core/tracking.rs`
  Purpose: Add the evaluation schema, event/outcome types, write APIs, read/summary APIs, and hermetic tests.

- Modify: `src/core/runner.rs`
  Purpose: Record default evaluation outcomes from the common execution skeleton so most commands are covered automatically.

- Modify: `src/main.rs`
  Purpose: Add the `rtk gain --evaluation` CLI flag and wire parse-fallback/hard-failure flows to explicit evaluation outcomes.

- Modify: `src/analytics/gain.rs`
  Purpose: Render an evaluation report with total uses, success/failure counts, compression rate, and recent/top failures.

- Optional doc touch: `src/core/README.md` or `README.md`
  Purpose: Briefly document the temporary evaluation toggle and the new `rtk gain --evaluation` report.

## Outcome Semantics

Use these exact meanings so the report stays honest:

- `compressed`: RTK ran normally and produced a lossy summary view.
- `preserved_raw`: RTK ran normally and intentionally preserved raw output.
- `passthrough_expected`: RTK intentionally bypassed compression, such as explicit passthrough/proxy behavior.
- `fallback_recovered`: RTK missed native handling and recovered by falling back to raw execution.
- `hard_failure`: RTK could not complete handling or fallback and returned an RTK-side failure.

Report these aggregates:

- `total_uses = all evaluation events`
- `successful_uses = compressed + preserved_raw + passthrough_expected`
- `failed_uses = fallback_recovered + hard_failure`
- `compression_attempts = compressed + preserved_raw`
- `compression_success_rate = compressed / compression_attempts`

### Task 1: Add the Toggle and Evaluation Model

**Files:**
- Modify: `src/core/config.rs`
- Modify: `src/core/tracking.rs`
- Test: `src/core/tracking.rs`

- [ ] **Step 1: Write the failing config and model tests**

Add these tests near the existing config and tracking tests first:

```rust
#[test]
fn test_evaluation_config_default_disabled() {
    let config = Config::default();
    assert!(!config.evaluation.enabled);
}

#[test]
fn test_evaluation_config_deserializes() {
    let toml = r#"
[evaluation]
enabled = true
"#;
    let config: Config = toml::from_str(toml).expect("valid toml");
    assert!(config.evaluation.enabled);
}

#[test]
fn test_record_evaluation_event_roundtrip() {
    with_tracking_test_db(|| {
        let tracker = Tracker::new().expect("tracker");
        tracker
            .record_evaluation(
                "git diff",
                "rtk git diff",
                EvaluationOutcome::Compressed,
                true,
                None,
            )
            .expect("record evaluation");

        let summary = tracker.get_evaluation_summary_filtered(None).expect("summary");
        assert_eq!(summary.total_uses, 1);
        assert_eq!(summary.successful_uses, 1);
        assert_eq!(summary.failed_uses, 0);
        assert_eq!(summary.compressed_uses, 1);
    });
}
```

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run:

```powershell
cargo test evaluation_config
cargo test record_evaluation_event_roundtrip
```

Expected: compile errors for missing `evaluation` config and missing evaluation tracking APIs/types.

- [ ] **Step 3: Add the config section and evaluation types**

Add this shape in `src/core/config.rs`:

```rust
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub tracking: TrackingConfig,
    #[serde(default)]
    pub evaluation: EvaluationConfig,
    #[serde(default)]
    pub display: DisplayConfig,
    // ...
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EvaluationConfig {
    pub enabled: bool,
}

impl Default for EvaluationConfig {
    fn default() -> Self {
        Self { enabled: false }
    }
}

pub fn evaluation_enabled() -> Option<bool> {
    Config::load().ok().map(|c| c.evaluation.enabled)
}
```

Add these types in `src/core/tracking.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluationOutcome {
    Compressed,
    PreservedRaw,
    PassthroughExpected,
    FallbackRecovered,
    HardFailure,
}

impl EvaluationOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Compressed => "compressed",
            Self::PreservedRaw => "preserved_raw",
            Self::PassthroughExpected => "passthrough_expected",
            Self::FallbackRecovered => "fallback_recovered",
            Self::HardFailure => "hard_failure",
        }
    }
}

#[derive(Debug)]
pub struct EvaluationSummary {
    pub total_uses: usize,
    pub successful_uses: usize,
    pub failed_uses: usize,
    pub compressed_uses: usize,
    pub preserved_raw_uses: usize,
    pub passthrough_uses: usize,
    pub recovered_failures: usize,
    pub hard_failures: usize,
    pub compression_success_rate: f64,
    pub top_failed_commands: Vec<(String, usize)>,
    pub recent_failures: Vec<(String, String, String)>,
}
```

- [ ] **Step 4: Add the SQLite table and tracker APIs**

Extend `Tracker::new()` with a new table and indexes:

```rust
conn.execute(
    "CREATE TABLE IF NOT EXISTS evaluation_events (
        id INTEGER PRIMARY KEY,
        timestamp TEXT NOT NULL,
        original_cmd TEXT NOT NULL,
        rtk_cmd TEXT NOT NULL,
        project_path TEXT DEFAULT '',
        outcome TEXT NOT NULL,
        compression_attempted INTEGER NOT NULL DEFAULT 0,
        failure_reason TEXT
    )",
    [],
)?;

conn.execute(
    "CREATE INDEX IF NOT EXISTS idx_eval_project_timestamp
     ON evaluation_events(project_path, timestamp)",
    [],
)?;
```

Add tracker methods:

```rust
pub fn record_evaluation(
    &self,
    original_cmd: &str,
    rtk_cmd: &str,
    outcome: EvaluationOutcome,
    compression_attempted: bool,
    failure_reason: Option<&str>,
) -> Result<()>;

pub fn get_evaluation_summary_filtered(
    &self,
    project_path: Option<&str>,
) -> Result<EvaluationSummary>;
```

Inside `record_evaluation`, reuse `current_project_path_string()` and `cleanup_old()`.

- [ ] **Step 5: Run the targeted tests to verify they pass**

Run:

```powershell
cargo test evaluation_config
cargo test record_evaluation_event_roundtrip
```

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/core/config.rs src/core/tracking.rs
git commit -m "feat: add toggleable evaluation tracking model"
```

### Task 2: Centralize Default Evaluation Logging in the Tracking Layer

**Files:**
- Modify: `src/core/tracking.rs`
- Test: `src/core/tracking.rs`

- [ ] **Step 1: Write the failing behavior tests for automatic outcome derivation**

Add tests that lock in the default mapping:

```rust
#[test]
fn test_track_records_compressed_evaluation_outcome() {
    with_tracking_test_db(|| {
        let timer = TimedExecution::start();
        timer.track("cargo test", "rtk cargo test", "raw output", "[summary view]\nok");

        let tracker = Tracker::new().expect("tracker");
        let summary = tracker.get_evaluation_summary_filtered(None).expect("summary");
        assert_eq!(summary.compressed_uses, 1);
        assert_eq!(summary.successful_uses, 1);
    });
}

#[test]
fn test_track_passthrough_records_passthrough_evaluation_outcome() {
    with_tracking_test_db(|| {
        let timer = TimedExecution::start();
        timer.track_passthrough("git tag", "rtk git tag (passthrough)");

        let tracker = Tracker::new().expect("tracker");
        let summary = tracker.get_evaluation_summary_filtered(None).expect("summary");
        assert_eq!(summary.passthrough_uses, 1);
        assert_eq!(summary.failed_uses, 0);
    });
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:

```powershell
cargo test track_records_compressed_evaluation_outcome
cargo test track_passthrough_records_passthrough_evaluation_outcome
```

Expected: FAIL because `TimedExecution` does not yet write evaluation events.

- [ ] **Step 3: Add silent helpers to `TimedExecution`**

Implement one shared helper so existing call sites keep working:

```rust
impl TimedExecution {
    fn record_with_outcome(
        &self,
        original_cmd: &str,
        rtk_cmd: &str,
        input_tokens: usize,
        output_tokens: usize,
        outcome: EvaluationOutcome,
        compression_attempted: bool,
        failure_reason: Option<&str>,
    ) {
        let elapsed_ms = self.start.elapsed().as_millis() as u64;
        if let Ok(tracker) = Tracker::new() {
            let _ = tracker.record(original_cmd, rtk_cmd, input_tokens, output_tokens, elapsed_ms);
            let _ = tracker.record_evaluation(
                original_cmd,
                rtk_cmd,
                outcome,
                compression_attempted,
                failure_reason,
            );
        }
    }
}
```

Then update:

```rust
pub fn track(&self, original_cmd: &str, rtk_cmd: &str, input: &str, output: &str) {
    let input_tokens = estimate_tokens(input);
    let output_tokens = estimate_tokens(output);
    let compression_attempted = input_tokens > 0;
    let outcome = if input_tokens > output_tokens {
        EvaluationOutcome::Compressed
    } else {
        EvaluationOutcome::PreservedRaw
    };
    self.record_with_outcome(
        original_cmd,
        rtk_cmd,
        input_tokens,
        output_tokens,
        outcome,
        compression_attempted,
        None,
    );
}

pub fn track_passthrough(&self, original_cmd: &str, rtk_cmd: &str) {
    self.record_with_outcome(
        original_cmd,
        rtk_cmd,
        0,
        0,
        EvaluationOutcome::PassthroughExpected,
        false,
        None,
    );
}
```

Add one explicit method for fallback flows:

```rust
pub fn track_passthrough_outcome(
    &self,
    original_cmd: &str,
    rtk_cmd: &str,
    outcome: EvaluationOutcome,
    failure_reason: Option<&str>,
) {
    self.record_with_outcome(original_cmd, rtk_cmd, 0, 0, outcome, false, failure_reason);
}
```

- [ ] **Step 4: Run the targeted tests again**

Run:

```powershell
cargo test track_records_compressed_evaluation_outcome
cargo test track_passthrough_records_passthrough_evaluation_outcome
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/core/tracking.rs
git commit -m "feat: record evaluation outcomes from timed tracking"
```

### Task 3: Wire Parse Fallback and Hard Failure Paths Explicitly

**Files:**
- Modify: `src/main.rs`
- Test: `src/core/tracking.rs`
- Test: `src/main.rs`

- [ ] **Step 1: Write the failing test for recovered fallback classification**

Add a tracking test like:

```rust
#[test]
fn test_track_passthrough_outcome_records_fallback_recovered() {
    with_tracking_test_db(|| {
        let timer = TimedExecution::start();
        timer.track_passthrough_outcome(
            "unknowncmd --flag",
            "rtk fallback: unknowncmd --flag",
            EvaluationOutcome::FallbackRecovered,
            Some("parse failure"),
        );

        let tracker = Tracker::new().expect("tracker");
        let summary = tracker.get_evaluation_summary_filtered(None).expect("summary");
        assert_eq!(summary.recovered_failures, 1);
        assert_eq!(summary.failed_uses, 1);
    });
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```powershell
cargo test track_passthrough_outcome_records_fallback_recovered
```

Expected: FAIL until the explicit fallback path is wired.

- [ ] **Step 3: Update `main.rs` parse-error fallback branches**

In the parse-error recovery branch, replace:

```rust
timer.track_passthrough(&raw_command, &format!("rtk fallback: {}", raw_command));
```

with:

```rust
timer.track_passthrough_outcome(
    &raw_command,
    &format!("rtk fallback: {}", raw_command),
    core::tracking::EvaluationOutcome::FallbackRecovered,
    Some("clap parse fallback"),
);
```

In the hard failure branch, add a direct evaluation write before returning:

```rust
if let Ok(tracker) = core::tracking::Tracker::new() {
    let _ = tracker.record_evaluation(
        &raw_command,
        &format!("rtk fallback: {}", raw_command),
        core::tracking::EvaluationOutcome::HardFailure,
        false,
        Some("fallback exec failed"),
    );
}
```

Do not remove the existing `record_parse_failure_silent` calls. They serve a different report.

- [ ] **Step 4: Run the targeted test**

Run:

```powershell
cargo test track_passthrough_outcome_records_fallback_recovered
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/core/tracking.rs
git commit -m "feat: classify fallback and hard failures in evaluation logs"
```

### Task 4: Expose the Report Through `rtk gain --evaluation`

**Files:**
- Modify: `src/main.rs`
- Modify: `src/analytics/gain.rs`
- Test: `src/main.rs`
- Test: `src/analytics/gain.rs`

- [ ] **Step 1: Write the failing CLI and formatting tests**

Add a CLI parse test:

```rust
#[test]
fn test_gain_evaluation_flag_parses() {
    let result = Cli::try_parse_from(["rtk", "gain", "--evaluation"]);
    assert!(result.is_ok());
    match result.unwrap().command {
        Commands::Gain { evaluation, .. } => assert!(evaluation),
        _ => panic!("Expected Gain command"),
    }
}
```

Add a formatting test in `src/analytics/gain.rs`:

```rust
#[test]
fn test_format_evaluation_report_shows_core_counts() {
    let summary = EvaluationSummary {
        total_uses: 20,
        successful_uses: 18,
        failed_uses: 2,
        compressed_uses: 12,
        preserved_raw_uses: 4,
        passthrough_uses: 2,
        recovered_failures: 1,
        hard_failures: 1,
        compression_success_rate: 75.0,
        top_failed_commands: vec![("unknowncmd".into(), 2)],
        recent_failures: vec![("now".into(), "unknowncmd".into(), "clap parse fallback".into())],
    };

    let text = format_evaluation_report(&summary, None);
    assert!(text.contains("Total RTK uses"));
    assert!(text.contains("Successful uses"));
    assert!(text.contains("Failed uses"));
    assert!(text.contains("Compression success rate"));
    assert!(text.contains("unknowncmd"));
    assert!(text.contains("clap parse fallback"));
}
```

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run:

```powershell
cargo test gain_evaluation_flag_parses
cargo test format_evaluation_report_shows_core_counts
```

Expected: FAIL because the CLI flag and report do not exist yet.

- [ ] **Step 3: Add the CLI flag and reporting path**

In `src/main.rs`, extend `Commands::Gain`:

```rust
Gain {
    #[arg(short, long)]
    project: bool,
    #[arg(long)]
    evaluation: bool,
    // existing flags...
}
```

Pass the new flag into `analytics::gain::run(...)`.

In `src/analytics/gain.rs`, short-circuit before the normal token report:

```rust
if evaluation {
    return show_evaluation(&tracker, project_scope.as_deref());
}
```

Add a renderer:

```rust
fn show_evaluation(tracker: &Tracker, project_scope: Option<&str>) -> Result<()> {
    let summary = tracker.get_evaluation_summary_filtered(project_scope)?;
    println!("{}", format_evaluation_report(&summary, project_scope));
    Ok(())
}
```

Format it with these sections:

```text
RTK Evaluation
Total RTK uses
Successful uses
Failed uses
Compression attempts
Compression success rate
Breakdown by outcome
Top failed commands
Recent failures
```

- [ ] **Step 4: Run the targeted tests again**

Run:

```powershell
cargo test gain_evaluation_flag_parses
cargo test format_evaluation_report_shows_core_counts
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/analytics/gain.rs
git commit -m "feat: add evaluation report to rtk gain"
```

### Task 5: Document the Toggle and Verify End-to-End

**Files:**
- Modify: `src/core/README.md`
- Test: `src/core/tracking.rs`
- Test: `src/analytics/gain.rs`

- [ ] **Step 1: Add a short doc section**

Add a concise note like:

```md
## Evaluation Logger

Temporary rollout/evaluation metric collection can be enabled in `config.toml`:

```toml
[evaluation]
enabled = true
```

Then inspect the current report with:

```bash
rtk gain --evaluation
rtk gain --evaluation --project
```
```

- [ ] **Step 2: Add the final integration tests**

Add one tracking integration test that records all five outcomes and verifies the aggregate math:

```rust
#[test]
fn test_evaluation_summary_counts_success_and_failure_correctly() {
    with_tracking_test_db(|| {
        let tracker = Tracker::new().expect("tracker");
        tracker.record_evaluation("a", "rtk a", EvaluationOutcome::Compressed, true, None).unwrap();
        tracker.record_evaluation("b", "rtk b", EvaluationOutcome::PreservedRaw, true, None).unwrap();
        tracker.record_evaluation("c", "rtk c", EvaluationOutcome::PassthroughExpected, false, None).unwrap();
        tracker.record_evaluation("d", "rtk d", EvaluationOutcome::FallbackRecovered, false, Some("parse")).unwrap();
        tracker.record_evaluation("e", "rtk e", EvaluationOutcome::HardFailure, false, Some("spawn")).unwrap();

        let summary = tracker.get_evaluation_summary_filtered(None).unwrap();
        assert_eq!(summary.total_uses, 5);
        assert_eq!(summary.successful_uses, 3);
        assert_eq!(summary.failed_uses, 2);
        assert_eq!(summary.compression_success_rate, 50.0);
    });
}
```

- [ ] **Step 3: Run the full verification pipeline**

Run:

```powershell
cargo fmt --all
cargo clippy --all-targets
cargo test --all
```

Expected:

```text
Finished `dev` profile ...
test result: ok. ... passed; 0 failed
```

- [ ] **Step 4: Smoke the feature manually**

Run:

```powershell
rtk gain --evaluation
rtk git log --oneline -1
rtk gain --evaluation
```

Expected:

```text
Total RTK uses: 1
Successful uses: 1
Failed uses: 0
```

Then force a fallback path and re-run:

```powershell
rtk unknowncmd --flag
rtk gain --evaluation
```

Expected: failed or recovered count increments by 1.

- [ ] **Step 5: Commit**

```bash
git add src/core/README.md src/core/tracking.rs src/analytics/gain.rs
git commit -m "docs: document temporary evaluation logger"
```

## Self-Review

- Spec coverage: this plan covers toggle, schema, centralized writes, fallback classification, report surface, docs, and verification.
- Placeholder scan: no TODO/TBD markers remain.
- Type consistency: `EvaluationOutcome`, `EvaluationSummary`, `record_evaluation`, `get_evaluation_summary_filtered`, and `track_passthrough_outcome` are named consistently across tasks.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-07-evaluation-logger.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
