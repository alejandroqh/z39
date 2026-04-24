use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use anyhow::Result;

/// Find the z3 binary
/// Search order:
/// 1. Z3_BIN env var
/// 2. Same directory as the z39 binary (distribution layout)
/// 3. PATH lookup
pub fn find_z3() -> anyhow::Result<PathBuf> {
    // 1. Z3_BIN env var
    if let Ok(p) = std::env::var("Z3_BIN") {
        let pb = PathBuf::from(&p);
        if pb.exists() { return Ok(pb); }
    }

    // 2. Same directory as the z39 binary (distribution layout)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let z3_next_to_exe = dir.join("z3");
            if z3_next_to_exe.exists() { return Ok(z3_next_to_exe); }
            // Windows
            let z3_exe = dir.join("z3.exe");
            if z3_exe.exists() { return Ok(z3_exe); }
        }
    }

    // 3. PATH lookup
    for dir in std::env::var("PATH").unwrap_or_default().split(':') {
        let p = PathBuf::from(dir).join("z3");
        if p.exists() { return Ok(p); }
    }

    anyhow::bail!("z3 not found: install z3, set Z3_BIN, or place z3 binary next to z39")
}

#[derive(Debug, Clone, PartialEq)]
pub enum SolveStatus {
    Sat,
    Unsat,
    Unknown,
    Timeout,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct SolveResult {
    pub status: SolveStatus,
    pub model: Option<String>,
    pub raw_output: String,
    pub duration_ms: u64,
}

impl SolveResult {
    /// Token-optimized compact output
    pub fn to_compact(&self) -> String {
        let t = format!("{:.1}s", self.duration_ms as f64 / 1000.0);
        match &self.status {
            SolveStatus::Sat => {
                let m = self.model.as_deref().unwrap_or("");
                if m.is_empty() { format!("sat {t}") } else { format!("sat {m} {t}") }
            }
            SolveStatus::Unsat => format!("unsat {t}"),
            SolveStatus::Unknown => format!("unknown {t}"),
            SolveStatus::Timeout => format!("timeout {t}"),
            SolveStatus::Error(e) => format!("error {e} {t}"),
        }
    }

    /// Is the result sat?
    pub fn is_sat(&self) -> bool { self.status == SolveStatus::Sat }

    /// Is the result unsat?
    pub fn is_unsat(&self) -> bool { self.status == SolveStatus::Unsat }
}

/// Run z3 on SMT-LIB2 input
pub async fn solve(z3_bin: &PathBuf, smt_input: &str, timeout_secs: u64) -> SolveResult {
    let start = std::time::Instant::now();
    let dur = if timeout_secs > 0 {
        Duration::from_secs(timeout_secs)
    } else {
        Duration::from_secs(300)
    };

    let result = timeout(dur, run_z3_process(z3_bin, smt_input)).await;
    let elapsed = start.elapsed().as_millis() as u64;

    match result {
        Ok(Ok(output)) => parse_z3_output(&output, elapsed),
        Ok(Err(e)) => SolveResult {
            status: SolveStatus::Error(e.to_string()),
            model: None,
            raw_output: String::new(),
            duration_ms: elapsed,
        },
        Err(_) => SolveResult {
            status: SolveStatus::Timeout,
            model: None,
            raw_output: String::new(),
            duration_ms: elapsed,
        },
    }
}

async fn run_z3_process(z3_bin: &PathBuf, smt_input: &str) -> Result<String> {
    let mut child = Command::new(z3_bin)
        .arg("-in")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(smt_input.as_bytes()).await?;
        stdin.shutdown().await?;
    }

    let output = child.wait_with_output().await?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_z3_output(raw: &str, elapsed_ms: u64) -> SolveResult {
    let trimmed = raw.trim();
    let first_line = trimmed.lines().next().unwrap_or("").trim();

    let (status, model) = if first_line.starts_with("sat") || first_line == "sat" {
        (SolveStatus::Sat, extract_model(trimmed))
    } else if first_line.starts_with("unsat") {
        (SolveStatus::Unsat, None)
    } else if first_line.starts_with("unknown") {
        (SolveStatus::Unknown, None)
    } else if first_line.starts_with("error") {
        (SolveStatus::Error(first_line.to_string()), None)
    } else {
        (SolveStatus::Error(format!("unexpected: {first_line}")), None)
    };

    SolveResult { status, model, raw_output: trimmed.to_string(), duration_ms: elapsed_ms }
}

fn extract_model(output: &str) -> Option<String> {
    let mut assignments = Vec::new();
    let lines: Vec<&str> = output.lines().collect();
    for i in 0..lines.len() {
        let line = lines[i].trim();
        if line.starts_with("(define-fun") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            // parts: [(define-fun, name, (), Sort, ...]
            if parts.len() >= 4 {
                let name = parts[1]; // NOT parts[2]
                // Value on same line
                if parts.len() >= 5 {
                    let val = parts[parts.len() - 1].trim_end_matches(')');
                    if !val.is_empty() && val != "(" && val != "()" {
                        assignments.push(format!("{name}={val}"));
                        continue;
                    }
                }
                // Value on next line
                if i + 1 < lines.len() {
                    let next = lines[i + 1].trim();
                    let val = next.trim_end_matches(')');
                    if !val.is_empty() && val != "(" {
                        assignments.push(format!("{name}={val}"));
                    }
                }
            }
        }
    }
    if assignments.is_empty() { None } else { Some(assignments.join(" ")) }
}