# z39

[![Powered by Z3](https://img.shields.io/badge/powered%20by-Z3-blue)](https://github.com/Z3Prover/z3)

**Z3-powered reasoning for AI agents. MCP server + CLI for scheduling, logic, config, and safety.**

> The missing link between LLM reasoning and formal verification.

<details open>
<summary><b>Plain English</b></summary>

z39 helps AI assistants answer tricky yes-or-no questions with certainty instead of a guess. Give it a problem like "can all these meetings fit in my day?" or "is this action safe to run?" and it checks the rules for you rather than making something up.

</details>

<details>
<summary><b>For developers</b></summary>

z39 is a single Rust binary that bundles a CLI and an MCP server over the Z3 solver. Call it from a shell script or drop it into an MCP config: structured JSON in, compact answers out. No daemon, no FFI, no separate Z3 install.

</details>

<details>
<summary><b>For formal methods readers</b></summary>

z39 exposes Z3 through four domain encoders (scheduling, boolean logic, configuration, and safety), each translating structured inputs into SMT-LIB2 (QF_LIA / QF_LRA / QF_UF) and interpreting models back into domain language. Decision problems framed as satisfiability: feasible / infeasible, valid / counterexample, consistent / conflicting.

</details>

<details>
<summary><b>For both</b></summary>

z39 is a Rust CLI + stdio MCP server that marshals typed JSON into SMT-LIB2, spawns Z3 as a subprocess under a timeout, and parses models into token-efficient output. Four domain encoders (schedule → QF_LIA ordering/no-overlap, logic → QF_UF over declared sorts, config → QF_LIA + enum expansion, safety → pure Rust) plus a raw-SMT escape hatch; async job lifecycle is MCP-only since one-shot CLI invocations can't hold it.

</details>

## Why z39

LLMs understand messy human language. Z3 verifies precise logical constraints. Together they make AI agents more reliable: the agent doesn't just produce plausible answers, it can **prove** whether something is possible, impossible, equivalent, or unsafe.

- **CLI for scripting**: `z39 schedule`, `z39 logic`, `z39 config`, `z39 safety`, `z39 solve`. One-shot invocations from a shell, Makefile, or test harness.
- **MCP server on demand**: `z39 mcp` starts the MCP server over STDIO. No daemon between calls.
- **Single binary, auto-provisioned Z3**: ships one executable. Z3 is downloaded automatically on first run if it isn't already installed.
- **Subprocess isolation**: Z3 runs as a spawned subprocess (no FFI, no linking, crash-contained).

## Install

```bash
cargo install z39-solver
```

The crate is published as `z39-solver`; the installed binary is `z39`.

### Install for any AI CLI / IDE

Installs the binary and auto-configures it for every MCP client detected: **Claude Code**, **Claude Desktop**, **Codex**, **OpenCode**, **OpenClaw**.

```sh
curl -fsSL https://raw.githubusercontent.com/alejandroqh/marketplace/main/h39.sh | bash
```

On first run, z39 auto-provisions Z3: it downloads the official binary on macOS/Windows, or builds from source on Linux. No separate Z3 install needed.

### Build from source

```bash
./build
```

This builds z39 for all supported platforms and bundles Z3 automatically. Each distribution package includes both `z39` and `z3` in the same directory.

### Z3 discovery order

`solver::find_or_download_z3` resolves in this order:
1. `Z3_BIN` env var
2. A `z3` binary sibling next to the running `z39` binary (how the `./build` release archives work)
3. `~/.local/share/z39/z3` (auto-downloaded cache)
4. `PATH`
5. Fall back to downloading (macOS/Windows) or building from source (Linux)

---

## CLI

All subcommands that run Z3 accept a payload positionally, via `--file <path>`, or via `-` for stdin. The `mcp` subcommand starts the MCP server and takes no payload.

### `z39 schedule`: Is this schedule feasible?

```bash
z39 schedule '{
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
}'
```

```
feasible
standup 09:30-10:00
deep_work 10:00-12:00
lunch 12:00-13:00
review 13:00-13:45
```

| Flag | Default | Description |
|------|---------|-------------|
| `<INPUT>` / `--file` / `-` | required | JSON payload |
| `--timeout` | 30 | Solver timeout (seconds) |

### `z39 logic`: Verify boolean logic

```bash
z39 logic '{
  "description": "Is (x AND y) the same as (x OR y)?",
  "check": {
    "type": "find_counterexample",
    "vars": ["x:Bool", "y:Bool"],
    "condition": "(and x y)"
  }
}'
# → counterexample: x=false y=true
```

Check types: `always_true`, `equivalent`, `find_counterexample`, `consistent`, `find_satisfying`.

| Flag | Default | Description |
|------|---------|-------------|
| `<INPUT>` / `--file` / `-` | required | JSON payload |
| `--timeout` | 30 | Solver timeout (seconds) |

### `z39 config`: Validate configuration constraints

```bash
z39 config --file deployment_rules.json
```

Modes: `validate`, `find_valid`, `find_violation`. Var types: `bool`, `int {min,max}`, `enum`.

| Flag | Default | Description |
|------|---------|-------------|
| `<INPUT>` / `--file` / `-` | required | JSON payload |
| `--timeout` | 15 | Solver timeout (seconds) |

### `z39 safety`: Pre-check an action

Purely Rust-side (doesn't invoke Z3). Useful from shell hooks before an agent runs a tool.

```bash
z39 safety '{
  "action": {"kind": "file_delete", "target": "/etc/passwd", "destructive": true},
  "protected": ["/etc", "/var", "/boot"]
}'
# → unsafe — BLOCKED: '/etc/passwd' is a protected resource and the action is destructive
```

Action kinds: `file_read`, `file_write`, `file_delete`, `command_exec`, `network_request`, `send_message`.

### `z39 solve`: Raw SMT-LIB2

```bash
z39 solve '(declare-const x Int)(assert (> x 5))(assert (< x 10))(check-sat)(get-model)'
# → sat x=6 0.0s
```

| Flag | Default | Description |
|------|---------|-------------|
| `<FORMULA>` / `--file` / `-` | required | SMT-LIB2 formula |
| `--timeout` | 30 | Solver timeout (seconds) |

### `z39 mcp`: Start the MCP server

```bash
z39 mcp
```

STDIO transport. Normally invoked by an MCP client, not a human. See the MCP section below.

---

## MCP Server

`z39 mcp` starts the MCP server over STDIO. It exposes all the domain tools from the CLI **plus** async solve tools that only make sense inside a long-lived MCP session.

### Configuration

```json
{
  "mcpServers": {
    "z39": {
      "type": "stdio",
      "command": "z39",
      "args": ["mcp"]
    }
  }
}
```

z39 finds z3 automatically: same directory as the z39 binary, `Z3_BIN` env var, `~/.local/share/z39/z3` (auto-downloaded), or PATH.

### Tools

#### Domain-specific

| Tool | Human question | What Z3 does |
|------|----------------|---------------|
| `z39_schedule` | "Can I fit 5 meetings + lunch in my day?" | Scheduling with ordering, overlap, time-window constraints |
| `z39_logic` | "Are these access rules equivalent?" / "Find me a counterexample" | Boolean-logic verification, equivalence, consistency |
| `z39_config` | "Do these deployment rules conflict?" | Configuration validation, constraint satisfaction |
| `z39_safety` | "Is it safe to delete /etc/passwd?" | Pre-check actions against protected resources |

#### Low-level (advanced)

| Tool | Purpose |
|------|---------|
| `z39_solve` | Raw SMT-LIB2 → compact result |
| `z39_solve_async` | Long-running solve (returns `job_id`) |
| `z39_job_status` | Poll async job |
| `z39_job_result` | Get async job result |
| `z39_job_cancel` | Cancel async job |

Async jobs live in memory for the lifetime of the MCP server process. They are not exposed from the CLI because each CLI invocation is a fresh process.

## Example: solving a Sudoku

With `z39` configured as an MCP server, paste this prompt into any MCP-capable agent:

````
Solve this Sudoku using z39:

2 . 5 | . . 7 | . . 6
4 . . | 9 6 . | . 2 .
. . . | . 8 . | . 4 5
------+-------+------
9 8 . | . 7 4 | . . .
5 7 . | 8 . 2 | . 6 9
. . . | 6 3 . | . 5 7
------+-------+------
7 5 . | . 2 . | . . .
. 6 . | . 5 1 | . . 2
3 . . | 4 . . | 5 . 8
````

The agent encodes the puzzle as SMT-LIB2 (81 `Int` cells bounded 1–9, 27 `distinct` constraints for rows/columns/boxes, plus equality for each given) and calls `z39_solve`. Z3 returns the unique model:

```
2 3 5 | 1 4 7 | 9 8 6
4 1 8 | 9 6 5 | 7 2 3
6 9 7 | 2 8 3 | 1 4 5
------+-------+------
9 8 6 | 5 7 4 | 2 3 1
5 7 3 | 8 1 2 | 4 6 9
1 4 2 | 6 3 9 | 8 5 7
------+-------+------
7 5 9 | 3 2 8 | 6 1 4
8 6 4 | 7 5 1 | 3 9 2
3 2 1 | 4 9 6 | 5 7 8
```

Z3 solve time: **0.1s**. End-to-end with Opus 4.7 (reading the puzzle, encoding to SMT-LIB2, calling the tool, and formatting the output): **2m 17s**.

## Example: planning a busy day

A more everyday use. Paste into any MCP-capable agent:

````
Can I fit all of this between 9am and 6pm today?

- Morning standup (30 min)
- Deep work on the Q2 report (2 hours)
- Lunch (45 min, sometime after noon)
- Dentist appointment (30 min, any time from 2pm)
- 1:1 with my manager (30 min, has to come after the report)
- Pick up groceries (30 min)
````

The agent maps the request to `z39_schedule`, which encodes the durations, time windows, and ordering into QF_LIA and asks Z3 whether a valid arrangement exists:

```
feasible
standup     09:00-09:30
deep_work   09:30-11:30
lunch       12:00-12:45
dentist     14:00-14:30
one_on_one  14:30-15:00
groceries   15:00-15:30
```

If any constraint makes the day impossible (e.g. you add a 3-hour task with everything else already full), Z3 returns `infeasible` instead of the agent guessing.

## Example: who sits where at dinner

A real headache the moment you have more than a few guests. Humans can't reliably hold 8 people plus a handful of adjacency rules in their head; LLMs produce confident-looking seating charts that quietly violate a constraint. Z3 solves it instantly.

Paste into any MCP-capable agent:

````
Work out a seating order for 8 guests at a long table (seats 1–8):

- Alice and Bob are a couple → they must sit next to each other
- Carol and Dave just broke up → they must NOT sit next to each other
- Eve gets anxious in the middle → she needs an end seat (1 or 8)
- Frank and Grace are siblings → parents asked they sit together
- Henry and Bob have a work beef → they must NOT sit next to each other
- Dave is hosting → he sits somewhere in the middle, not at either end
````

The agent encodes positions as integers 1–8, adjacency as `|pos_a − pos_b| = 1`, and calls `z39_solve`. Z3 returns a valid assignment:

```
Seat 1: Eve
Seat 2: Carol
Seat 3: Bob
Seat 4: Alice
Seat 5: Dave
Seat 6: Frank
Seat 7: Grace
Seat 8: Henry
```

Z3 solve time: ~10ms. If you added one more rule that made the whole thing impossible (say, Henry also must sit next to Eve), Z3 would return `unsat` and the agent would tell you flat out that no arrangement exists — instead of inventing one that looks plausible.

## Output format

Token-optimized compact output:

| Status | Example |
|--------|---------|
| satisfiable | `sat x=1 y=1 8.2s` |
| unsatisfiable | `unsat 0.3s` |
| proven always true | `valid 0.3s` |
| timeout | `timeout 30.0s` |
| scheduling | `feasible\n<schedule>` / `infeasible — ...` |
| safety | `safe — OK: ...` / `unsafe — BLOCKED: ...` |

## Architecture

```
Human intent
  → AI translates to constraints
    → z39 encodes to SMT-LIB2
      → Z3 solves (subprocess)
        → AI explains
```

- Subprocess model: Z3 runs as async subprocess (no FFI, no linking, crash isolation).
- turbomcp 3.x: Rust MCP SDK with `#[server]` and `#[tool]` macros.
- clap derive: CLI surface.
- Single Tokio runtime: shared between CLI solves and MCP server.

## Environment

| Variable | Purpose |
|----------|---------|
| `Z3_BIN` | Override Z3 binary path. Otherwise looked up next to `z39`, in the auto-download cache, or on `PATH`. |

## License

Apache-2.0
