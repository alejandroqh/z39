# z39 — Z3-powered reasoning for AI agents

> **The missing link between LLM reasoning and formal verification.**

z39 is an MCP server that gives AI agents access to Z3's constraint solving capabilities through **domain-specific tools** — not raw SMT-LIB2.

## Why z39?

LLMs understand messy human language. Z3 verifies precise logical constraints. Together they make AI agents more reliable — the agent doesn't just produce plausible answers, it can **prove** whether something is possible, impossible, equivalent, or unsafe.

## Tools

### Domain-Specific (what agents actually need)

| Tool | Human Question | What Z3 Does |
|------|---------------|---------------|
| `z39_schedule` | "Can I fit 5 meetings + lunch in my day?" | Scheduling with ordering, overlap, time-window constraints |
| `z39_logic` | "Are these access rules equivalent?" / "Find me a counterexample" | Boolean logic verification, equivalence, consistency |
| `z39_config` | "Do these deployment rules conflict?" | Configuration validation, constraint satisfaction |
| `z39_safety` | "Is it safe to delete /etc/passwd?" | Pre-check actions against protected resources |

### Low-Level (for advanced use)

| Tool | Purpose |
|------|---------|
| `z39_solve` | Raw SMT-LIB2 → compact result |
| `z39_solve_async` | Long-running solve (returns job_id) |
| `z39_job_status` | Poll async job |
| `z39_job_result` | Get async job result |
| `z39_job_cancel` | Cancel async job |

## Quick Start

### Install Z3

```bash
# Option 1: Package manager
apt install z3  # or brew install z3

# Option 2: Build from source
git clone https://github.com/Z3Prover/z3.git /tmp/z3-build
cd /tmp/z3-build && python3 scripts/mk_make.py
cd build && make -j$(nproc)

# Option 3: Set Z3_BIN explicitly
export Z3_BIN=/path/to/z3
```

### Build z39

```bash
cargo build --release
```

### Configure MCP

Add to your `.mcp.json` or MCP client config:

```json
{
  "mcpServers": {
    "z39": {
      "type": "stdio",
      "command": "/path/to/z39/target/release/z39",
      "env": {
        "Z3_BIN": "/path/to/z3"
      }
    }
  }
}
```

## Examples

### Scheduling: "Can I fit these tasks?"

```json
{
  "tasks": [
    {"name": "standup", "duration": 30},
    {"name": "deep_work", "duration": 120},
    {"name": "lunch", "duration": 60},
    {"name": "review", "duration": 45}
  ],
  "slot_start": 540,
  "slot_end": 1020,
  "constraints": [
    {"type": "before", "a": "standup", "b": "deep_work"},
    {"type": "start_after", "task": "lunch", "time": 720}
  ]
}
```

Result:
```
feasible
standup 09:30-10:00
deep_work 10:00-12:00
lunch 12:00-13:00
review 13:00-13:45
```

### Logic: "Find a counterexample"

```json
{
  "description": "Is (x AND y) the same as (x OR y)?",
  "check": {
    "type": "find_counterexample",
    "vars": ["x:Bool", "y:Bool"],
    "condition": "(and x y)"
  }
}
```

Result:
```
counterexample: x=false y=true
```

### Safety: "Can I delete /etc/passwd?"

```json
{
  "action": {"kind": "file_delete", "target": "/etc/passwd", "destructive": true},
  "protected": ["/etc", "/var", "/boot"]
}
```

Result:
```
unsafe — BLOCKED: '/etc/passwd' is a protected resource and the action is destructive
```

## Output Format

z39 uses token-optimized compact output:
- `sat x=1 y=1 8.2s` — satisfiable with model and time
- `unsat 0.3s` — unsatisfiable (no solution)
- `valid 0.3s` — proven always true
- `timeout 30.0s` — Z3 timed out

## Architecture

```
Human intent → AI translates to constraints → z39 encodes to SMT-LIB2 → Z3 solves → AI explains
```

- **Subprocess model**: Z3 runs as async subprocess (no FFI, no linking, crash isolation)
- **Async jobs**: Long-running solves return job_id for polling
- **turbomcp 3.x**: Modern Rust MCP SDK with `#[server]` and `#[tool]` macros
- **Compact output**: Token-optimized for LLM consumption

## License

Apache-2.0