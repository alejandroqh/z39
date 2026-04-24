/// Safety domain: pre-check actions before agent execution
///
/// Before running commands, modifying files, or taking actions,
/// the agent can verify that the proposed action doesn't violate safety rules.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyCheckRequest {
    /// The action being considered
    pub action: ActionSpec,
    /// Safety rules that must hold (SMT-LIB2 boolean expressions)
    #[serde(default)]
    pub rules: Vec<String>,
    /// Protected resources (paths, commands, domains)
    #[serde(default)]
    pub protected: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionSpec {
    /// What type of action
    pub kind: ActionKind,
    /// Target of the action (file path, command, URL, etc.)
    pub target: String,
    /// Whether the action is destructive (delete, overwrite, send)
    #[serde(default)]
    pub destructive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    FileRead,
    FileWrite,
    FileDelete,
    CommandExec,
    NetworkRequest,
    SendMessage,
}

pub fn encode_safety(req: &SafetyCheckRequest) -> String {
    let mut s = String::new();
    s.push_str("(set-logic QF_LIA)\n");

    // Declare action variables
    s.push_str("(declare-const action_destructive Bool)\n");
    s.push_str("(declare-const target_protected Bool)\n");
    s.push_str("(declare-const is_write Bool)\n");
    s.push_str("(declare-const is_delete Bool)\n");
    s.push_str("(declare-const is_exec Bool)\n");
    s.push_str("(declare-const is_network Bool)\n");
    s.push_str("(declare-const is_message Bool)\n");
    s.push_str("(declare-const is_safe Bool)\n");

    // Encode the action
    if req.action.destructive {
        s.push_str("(assert action_destructive)\n");
    } else {
        s.push_str("(assert (not action_destructive))\n");
    }

    match req.action.kind {
        ActionKind::FileRead => {
            s.push_str("(assert (not is_write))\n");
            s.push_str("(assert (not is_delete))\n");
            s.push_str("(assert (not is_exec))\n");
        }
        ActionKind::FileWrite => {
            s.push_str("(assert is_write)\n");
            s.push_str("(assert (not is_delete))\n");
        }
        ActionKind::FileDelete => {
            s.push_str("(assert is_delete)\n");
            s.push_str("(assert is_write)\n"); // delete is also a write
        }
        ActionKind::CommandExec => {
            s.push_str("(assert is_exec)\n");
        }
        ActionKind::NetworkRequest => {
            s.push_str("(assert is_network)\n");
        }
        ActionKind::SendMessage => {
            s.push_str("(assert is_message)\n");
        }
    }

    // Check if target is protected
    if req.protected.iter().any(|p| req.action.target.contains(p)) {
        s.push_str("(assert target_protected)\n");
    } else {
        s.push_str("(assert (not target_protected))\n");
    }

    // Safety rules
    for r in &req.rules {
        s.push_str(&format!("(assert {r})\n"));
    }

    // Default safety invariants
    // Protected + destructive = unsafe
    s.push_str("(assert (= is_safe (not (and target_protected action_destructive))))\n");

    s.push_str("(check-sat)\n");
    s.push_str("(get-model)\n");
    s
}

pub fn interpret_safety(req: &SafetyCheckRequest) -> SafetyVerdict {
    let target_protected = req.protected.iter().any(|p| req.action.target.contains(p));
    let destructive = req.action.destructive;

    // Basic safety logic
    if target_protected && destructive {
        return SafetyVerdict {
            safe: false,
            reason: format!("BLOCKED: '{}' is a protected resource and the action is destructive", req.action.target),
        };
    }

    if target_protected && matches!(req.action.kind, ActionKind::FileDelete | ActionKind::FileWrite | ActionKind::CommandExec) {
        return SafetyVerdict {
            safe: false,
            reason: format!("CAUTION: '{}' is protected — {} requires approval", req.action.target, format_kind(&req.action.kind)),
        };
    }

    SafetyVerdict {
        safe: true,
        reason: format!("OK: {} on '{}' passes safety checks", format_kind(&req.action.kind), req.action.target),
    }
}

#[derive(Debug, Clone)]
pub struct SafetyVerdict {
    pub safe: bool,
    pub reason: String,
}

fn format_kind(kind: &ActionKind) -> &'static str {
    match kind {
        ActionKind::FileRead => "read",
        ActionKind::FileWrite => "write",
        ActionKind::FileDelete => "delete",
        ActionKind::CommandExec => "exec",
        ActionKind::NetworkRequest => "network",
        ActionKind::SendMessage => "message",
    }
}

pub fn run(payload: &str) -> Result<String, serde_json::Error> {
    let req: SafetyCheckRequest = serde_json::from_str(payload)?;
    let verdict = interpret_safety(&req);
    Ok(if verdict.safe {
        format!("safe — {}", verdict.reason)
    } else {
        format!("unsafe — {}", verdict.reason)
    })
}