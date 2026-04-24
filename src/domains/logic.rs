/// Logic domain: verify boolean logic, find counterexamples, check equivalence
///
/// The agent describes conditions as boolean expressions.
/// We encode them as Bool variables in Z3.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogicCheckRequest {
    /// Description of what to check
    pub description: String,
    /// Check type
    pub check: LogicCheckType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LogicCheckType {
    /// Check if a condition is always true (prove validity)
    #[serde(rename = "always_true")]
    AlwaysTrue {
        /// Variable declarations: ["x:Bool", "y:Bool", "a:Int"]
        vars: Vec<String>,
        /// The condition to check (SMT-LIB2 boolean expression)
        condition: String,
    },
    /// Check if two expressions are equivalent
    #[serde(rename = "equivalent")]
    Equivalent {
        vars: Vec<String>,
        expr_a: String,
        expr_b: String,
    },
    /// Find inputs that make condition false (counterexample)
    #[serde(rename = "find_counterexample")]
    FindCounterexample {
        vars: Vec<String>,
        condition: String,
    },
    /// Check if a set of rules is consistent (no contradictions)
    #[serde(rename = "consistent")]
    Consistent {
        vars: Vec<String>,
        rules: Vec<String>,
    },
    /// Find inputs that satisfy all conditions
    #[serde(rename = "find_satisfying")]
    FindSatisfying {
        vars: Vec<String>,
        conditions: Vec<String>,
    },
}

pub fn encode_logic(req: &LogicCheckRequest) -> String {
    match &req.check {
        LogicCheckType::AlwaysTrue { vars, condition } => {
            let mut s = String::new();
            s.push_str("(set-logic QF_LIA)\n");
            declare_vars(&mut s, vars);
            // Negate and check for unsat
            s.push_str(&format!("(assert (not {condition}))\n"));
            s.push_str("(check-sat)\n");
            s.push_str("(get-model)\n");
            s
        }
        LogicCheckType::Equivalent { vars, expr_a, expr_b } => {
            let mut s = String::new();
            s.push_str("(set-logic QF_LIA)\n");
            declare_vars(&mut s, vars);
            // Check if A != B is satisfiable (if unsat, they're equivalent)
            s.push_str(&format!("(assert (not (= {expr_a} {expr_b})))\n"));
            s.push_str("(check-sat)\n");
            s.push_str("(get-model)\n");
            s
        }
        LogicCheckType::FindCounterexample { vars, condition } => {
            let mut s = String::new();
            s.push_str("(set-logic QF_LIA)\n");
            declare_vars(&mut s, vars);
            s.push_str(&format!("(assert (not {condition}))\n"));
            s.push_str("(check-sat)\n");
            s.push_str("(get-model)\n");
            s
        }
        LogicCheckType::Consistent { vars, rules } => {
            let mut s = String::new();
            s.push_str("(set-logic QF_LIA)\n");
            declare_vars(&mut s, vars);
            for r in rules {
                s.push_str(&format!("(assert {r})\n"));
            }
            s.push_str("(check-sat)\n");
            s.push_str("(get-model)\n");
            s
        }
        LogicCheckType::FindSatisfying { vars, conditions } => {
            let mut s = String::new();
            s.push_str("(set-logic QF_LIA)\n");
            declare_vars(&mut s, vars);
            for c in conditions {
                s.push_str(&format!("(assert {c})\n"));
            }
            s.push_str("(check-sat)\n");
            s.push_str("(get-model)\n");
            s
        }
    }
}

/// Interpret logic check results in human-readable form
pub fn interpret_logic(req: &LogicCheckRequest, is_unsat: bool, model: Option<&str>) -> String {
    match &req.check {
        LogicCheckType::AlwaysTrue { .. } => {
            if is_unsat {
                "✓ valid — condition is always true".to_string()
            } else {
                let counter = model.unwrap_or("unknown");
                format!("✗ invalid — counterexample: {counter}")
            }
        }
        LogicCheckType::Equivalent { .. } => {
            if is_unsat {
                "✓ equivalent — expressions always produce the same result".to_string()
            } else {
                let counter = model.unwrap_or("unknown");
                format!("✗ not equivalent — difference found: {counter}")
            }
        }
        LogicCheckType::FindCounterexample { .. } => {
            if is_unsat {
                "no counterexample exists — condition is always true".to_string()
            } else {
                let cex = model.unwrap_or("unknown");
                format!("counterexample: {cex}")
            }
        }
        LogicCheckType::Consistent { .. } => {
            if is_unsat {
                "✗ inconsistent — rules contradict each other".to_string()
            } else {
                let m = model.unwrap_or("unknown");
                format!("✓ consistent — satisfying assignment: {m}")
            }
        }
        LogicCheckType::FindSatisfying { .. } => {
            if is_unsat {
                "✗ no solution — conditions are unsatisfiable".to_string()
            } else {
                let m = model.unwrap_or("unknown");
                format!("✓ solution found: {m}")
            }
        }
    }
}

fn declare_vars(s: &mut String, vars: &[String]) {
    for v in vars {
        if let Some((name, sort)) = v.split_once(':') {
            let z3_sort = match sort {
                "Bool" => "Bool",
                "Int" => "Int",
                "Real" => "Real",
                other => other,
            };
            s.push_str(&format!("(declare-const {name} {z3_sort})\n"));
        }
    }
}