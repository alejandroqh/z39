/// Config domain: validate configuration constraints (deployments, resources, permissions)
///
/// The agent describes config variables and invariants.
/// Z3 checks if the configuration is valid or finds conflicts.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigCheckRequest {
    /// Variables in the configuration
    pub vars: Vec<ConfigVar>,
    /// Invariant rules that must hold (SMT-LIB2 boolean expressions)
    pub rules: Vec<String>,
    /// What to check
    pub mode: ConfigCheckMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigVar {
    pub name: String,
    pub var_type: ConfigVarType,
    #[serde(default)]
    pub allowed_values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigVarType {
    Bool,
    Int { min: i32, max: i32 },
    Enum,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigCheckMode {
    /// Check if rules are consistent (no contradictions)
    Validate,
    /// Find a config that satisfies all rules
    FindValid,
    /// Find a config that violates at least one rule
    FindViolation,
}

pub fn encode_config(req: &ConfigCheckRequest) -> String {
    let mut s = String::new();
    s.push_str("(set-logic QF_LIA)\n");

    // Declare variables
    for v in &req.vars {
        match &v.var_type {
            ConfigVarType::Bool => {
                s.push_str(&format!("(declare-const {} Bool)\n", v.name));
            }
            ConfigVarType::Int { min, max } => {
                s.push_str(&format!("(declare-const {} Int)\n", v.name));
                s.push_str(&format!("(assert (>= {} {}))\n", v.name, min));
                s.push_str(&format!("(assert (<= {} {}))\n", v.name, max));
            }
            ConfigVarType::Enum => {
                // Encode as integer with allowed values
                s.push_str(&format!("(declare-const {} Int)\n", v.name));
                if !v.allowed_values.is_empty() {
                    let cases: Vec<String> = v.allowed_values.iter().enumerate()
                        .map(|(i, _)| format!("(= {} {})", v.name, i))
                        .collect();
                    s.push_str(&format!("(assert (or {}))\n", cases.join(" ")));
                }
            }
        }
    }

    match req.mode {
        ConfigCheckMode::Validate | ConfigCheckMode::FindValid => {
            for r in &req.rules {
                s.push_str(&format!("(assert {r})\n"));
            }
        }
        ConfigCheckMode::FindViolation => {
            // Assert that NOT all rules hold
            let negated: Vec<String> = req.rules.iter().map(|r| format!("(not {})", r)).collect();
            s.push_str(&format!("(assert (or {}))\n", negated.join(" ")));
        }
    }

    s.push_str("(check-sat)\n");
    s.push_str("(get-model)\n");
    s
}

pub fn interpret_config(req: &ConfigCheckRequest, is_unsat: bool, model: Option<&str>) -> String {
    match req.mode {
        ConfigCheckMode::Validate => {
            if is_unsat {
                "✗ invalid — rules are contradictory, no valid configuration exists".to_string()
            } else {
                let m = model.unwrap_or("unknown");
                format!("✓ valid — rules are consistent. Example config: {m}")
            }
        }
        ConfigCheckMode::FindValid => {
            if is_unsat {
                "✗ no valid configuration exists — rules are contradictory".to_string()
            } else {
                let m = model.unwrap_or("unknown");
                format!("✓ valid configuration: {m}")
            }
        }
        ConfigCheckMode::FindViolation => {
            if is_unsat {
                "✓ all rules always hold — no violation possible".to_string()
            } else {
                let m = model.unwrap_or("unknown");
                format!("⚠ violation found: {m}")
            }
        }
    }
}

pub async fn run(
    z3_bin: &std::path::PathBuf,
    payload: &str,
    timeout_secs: u64,
) -> Result<String, serde_json::Error> {
    let req: ConfigCheckRequest = serde_json::from_str(payload)?;
    let smt = encode_config(&req);
    let result = crate::solver::solve(z3_bin, &smt, timeout_secs).await;
    Ok(interpret_config(&req, result.is_unsat(), result.model.as_deref()))
}