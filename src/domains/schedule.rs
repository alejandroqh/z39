/// Schedule domain: translate scheduling problems into SMT-LIB2 for Z3
///
/// The agent describes tasks, time slots, and constraints.
/// We encode them as integer variables with ordering/overlap constraints.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleRequest {
    pub tasks: Vec<TaskSpec>,
    /// Time slot: available start and end (minutes from midnight)
    pub slot_start: i32,
    pub slot_end: i32,
    #[serde(default)]
    pub constraints: Vec<ScheduleConstraint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    pub name: String,
    pub duration: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ScheduleConstraint {
    #[serde(rename = "before")]
    Before { a: String, b: String },
    #[serde(rename = "no_overlap")]
    NoOverlap { tasks: Vec<String> },
    #[serde(rename = "start_after")]
    StartAfter { task: String, time: i32 },
    #[serde(rename = "finish_before")]
    FinishBefore { task: String, time: i32 },
    #[serde(rename = "sequence")]
    Sequence { tasks: Vec<String> },
}

pub fn encode_schedule(req: &ScheduleRequest) -> String {
    let mut s = String::new();
    s.push_str("(set-logic QF_LIA)\n");

    for t in &req.tasks {
        let v = sanitize_name(&t.name);
        s.push_str(&format!("(declare-const {v}_start Int)\n"));
        s.push_str(&format!("(declare-const {v}_end Int)\n"));
    }

    for t in &req.tasks {
        let v = sanitize_name(&t.name);
        s.push_str(&format!("(assert (>= {v}_start {}))\n", req.slot_start));
        s.push_str(&format!("(assert (<= {v}_end {}))\n", req.slot_end));
        s.push_str(&format!("(assert (= {v}_end (+ {v}_start {})))\n", t.duration));
    }

    // No-overlap between ALL tasks by default
    for i in 0..req.tasks.len() {
        for j in (i+1)..req.tasks.len() {
            let a = sanitize_name(&req.tasks[i].name);
            let b = sanitize_name(&req.tasks[j].name);
            s.push_str(&format!("(assert (or (<= {a}_end {b}_start) (<= {b}_end {a}_start)))\n"));
        }
    }

    for c in &req.constraints {
        match c {
            ScheduleConstraint::Before { a, b } => {
                let av = sanitize_name(a);
                let bv = sanitize_name(b);
                s.push_str(&format!("(assert (<= {av}_end {bv}_start))\n"));
            }
            ScheduleConstraint::NoOverlap { tasks } => {
                for i in 0..tasks.len() {
                    for j in (i+1)..tasks.len() {
                        let a = sanitize_name(&tasks[i]);
                        let b = sanitize_name(&tasks[j]);
                        s.push_str(&format!("(assert (or (<= {a}_end {b}_start) (<= {b}_end {a}_start)))\n"));
                    }
                }
            }
            ScheduleConstraint::StartAfter { task, time } => {
                let v = sanitize_name(task);
                s.push_str(&format!("(assert (>= {v}_start {time}))\n"));
            }
            ScheduleConstraint::FinishBefore { task, time } => {
                let v = sanitize_name(task);
                s.push_str(&format!("(assert (<= {v}_end {time}))\n"));
            }
            ScheduleConstraint::Sequence { tasks } => {
                for i in 0..tasks.len()-1 {
                    let a = sanitize_name(&tasks[i]);
                    let b = sanitize_name(&tasks[i+1]);
                    let dur = req.tasks.iter()
                        .find(|t| t.name == tasks[i])
                        .map(|t| t.duration)
                        .unwrap_or(0);
                    s.push_str(&format!("(assert (= {b}_start (+ {a}_start {dur})))\n"));
                }
            }
        }
    }

    s.push_str("(check-sat)\n");
    s.push_str("(get-model)\n");
    s
}

pub fn parse_schedule(model: &str, tasks: &[TaskSpec]) -> String {
    let mut schedule = Vec::new();
    for t in tasks {
        let v = sanitize_name(&t.name);
        let start = find_define_fun_val(model, &format!("{v}_start"));
        let end = find_define_fun_val(model, &format!("{v}_end"));
        if let (Some(s), Some(e)) = (start, end) {
            schedule.push(format!("{} {}-{}", t.name, format_time(s), format_time(e)));
        }
    }
    schedule.join("\n")
}

fn find_define_fun_val(model: &str, var_name: &str) -> Option<i32> {
    // Z3 model format (multi-line):
    //   (define-fun var_name () Int
    //     VALUE)
    // or single-line:
    //   (define-fun var_name () Sort VALUE)
    let lines: Vec<&str> = model.lines().collect();
    for i in 0..lines.len() {
        let line = lines[i].trim();
        if line.starts_with("(define-fun") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            // parts: [(define-fun, var_name, (), Sort, ...]
            if parts.len() >= 4 && parts[1] == var_name {
                // Check if value is on the same line
                if parts.len() >= 5 {
                    let val = parts[parts.len() - 1].trim_end_matches(')');
                    if let Ok(v) = val.parse::<i32>() { return Some(v); }
                }
                // Value on next line
                if i + 1 < lines.len() {
                    let next = lines[i + 1].trim();
                    let val = next.trim_end_matches(')');
                    if let Ok(v) = val.parse::<i32>() { return Some(v); }
                }
            }
        }
    }
    None
}

fn sanitize_name(name: &str) -> String {
    name.to_lowercase().replace(' ', "_").replace('-', "_")
}

fn format_time(minutes: i32) -> String {
    let h = minutes / 60;
    let m = minutes % 60;
    format!("{:02}:{:02}", h, m)
}

pub async fn run(
    z3_bin: &std::path::PathBuf,
    payload: &str,
    timeout_secs: u64,
) -> Result<String, serde_json::Error> {
    let req: ScheduleRequest = serde_json::from_str(payload)?;
    let smt = encode_schedule(&req);
    let result = crate::solver::solve(z3_bin, &smt, timeout_secs).await;
    if result.is_sat() {
        let schedule = parse_schedule(&result.raw_output, &req.tasks);
        Ok(format!("feasible\n{schedule}"))
    } else if result.is_unsat() {
        Ok("infeasible — constraints conflict, no valid schedule exists".to_string())
    } else {
        Ok(result.to_compact())
    }
}