# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2025-04-24

### Added

- MCP server with 9 tools: `z39_schedule`, `z39_logic`, `z39_config`, `z39_safety`, `z39_solve`, `z39_solve_async`, `z39_job_status`, `z39_job_result`, `z39_job_cancel`
- Domain-specific constraint encoding for scheduling, logic, config, and safety
- Async Z3 subprocess execution with job tracking (submit, poll, cancel)
- Token-optimized compact output format (`sat x=1 0.3s`, `unsat 0.1s`, etc.)
- Z3 binary discovery: `Z3_BIN` env, same-directory-as-binary, PATH lookup
- Cross-platform build script with Z3 bundling (from releases for macOS/Windows, from source for Linux)
- turbomcp 3.x SDK (MCP protocol 2025-06-18)
- Multi-line Z3 model parsing (`(define-fun var () Int VALUE)`)
- Support for Bool and Int variable types across all domains