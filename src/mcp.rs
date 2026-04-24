use std::path::PathBuf;
use std::sync::Arc;
use turbomcp::prelude::*;

use crate::{domains, job, solver};

// Compile-time check: MCP server version must match Cargo.toml
const _: () = {
    let cargo = env!("CARGO_PKG_VERSION").as_bytes();
    let mcp = b"1.0.0";
    assert!(
        cargo.len() == mcp.len(),
        "MCP server version does not match Cargo.toml — update #[server(version)] below"
    );
    let mut i = 0;
    while i < cargo.len() {
        assert!(
            cargo[i] == mcp[i],
            "MCP server version does not match Cargo.toml — update #[server(version)] below"
        );
        i += 1;
    }
};

#[derive(Clone)]
struct Z39Server {
    z3_bin: PathBuf,
    job_manager: Arc<job::JobManager>,
}

#[server(
    name = "z39",
    version = "1.0.0",
    description = "Z3-powered reasoning for AI agents — scheduling, logic, config, safety"
)]
impl Z39Server {
    #[tool(description = "Check if a schedule is feasible. Describe tasks with durations, time windows, and ordering/overlap constraints. Returns a valid schedule or reports conflicts.")]
    async fn z39_schedule(
        &self,
        #[description("JSON: {tasks:[{name,duration}],slot_start,slot_end,constraints}")]
        schedule: String,
    ) -> McpResult<String> {
        domains::schedule::run(&self.z3_bin, &schedule, 30)
            .await
            .map_err(|e| McpError::invalid_params(format!("invalid schedule JSON: {e}")))
    }

    #[tool(description = "Verify boolean logic: prove always-true, check equivalence, find counterexamples, check consistency.")]
    async fn z39_logic(
        &self,
        #[description("JSON: {description,check:{type,vars,condition/expr_a/expr_b/rules/conditions}}")]
        logic: String,
    ) -> McpResult<String> {
        domains::logic::run(&self.z3_bin, &logic, 30)
            .await
            .map_err(|e| McpError::invalid_params(format!("invalid logic JSON: {e}")))
    }

    #[tool(description = "Validate configuration constraints (deployment rules, resource limits, permissions). Check consistency, find valid config, or find violations.")]
    async fn z39_config(
        &self,
        #[description("JSON: {vars:[{name,var_type,allowed_values}],rules,mode}")]
        config: String,
    ) -> McpResult<String> {
        domains::config::run(&self.z3_bin, &config, 15)
            .await
            .map_err(|e| McpError::invalid_params(format!("invalid config JSON: {e}")))
    }

    #[tool(description = "Pre-check an action against safety rules. Checks if action targets protected resources or is destructive.")]
    async fn z39_safety(
        &self,
        #[description("JSON: {action:{kind,target,destructive},protected,rules}")]
        safety: String,
    ) -> McpResult<String> {
        domains::safety::run(&safety)
            .map_err(|e| McpError::invalid_params(format!("invalid safety JSON: {e}")))
    }

    #[tool(description = "Send raw SMT-LIB2 to Z3. For advanced use when domain tools don't cover the case.")]
    async fn z39_solve(
        &self,
        #[description("SMT-LIB2 formula")]
        formula: String,
        #[description("Timeout in seconds (default: 30)")]
        timeout: Option<u64>,
    ) -> McpResult<String> {
        let t = timeout.unwrap_or(30);
        let result = solver::solve(&self.z3_bin, &formula, t).await;
        Ok(result.to_compact())
    }

    #[tool(description = "Submit a long-running SMT-LIB2 solve asynchronously. Returns job_id for polling.")]
    async fn z39_solve_async(
        &self,
        #[description("SMT-LIB2 formula")]
        formula: String,
        #[description("Timeout in seconds (default: 60)")]
        timeout: Option<u64>,
    ) -> McpResult<String> {
        let t = timeout.unwrap_or(60);
        let id = self
            .job_manager
            .submit_with_bin("async_solve".to_string(), formula, t, self.z3_bin.clone())
            .await;
        Ok(format!("pending {id}"))
    }

    #[tool(description = "Check status of an async job.")]
    async fn z39_job_status(
        &self,
        #[description("Job ID from z39_solve_async")]
        job_id: String,
    ) -> McpResult<String> {
        match self.job_manager.status(&job_id) {
            Some(j) => Ok(format!("{:?} {:?}", j.status, j.label)),
            None => Ok(format!("not_found {job_id}")),
        }
    }

    #[tool(description = "Get the result of a completed async job.")]
    async fn z39_job_result(
        &self,
        #[description("Job ID from z39_solve_async")]
        job_id: String,
    ) -> McpResult<String> {
        match self.job_manager.result(&job_id) {
            Some(r) => Ok(r),
            None => Ok(format!("not_found {job_id}")),
        }
    }

    #[tool(description = "Cancel a running async job.")]
    async fn z39_job_cancel(
        &self,
        #[description("Job ID to cancel")]
        job_id: String,
    ) -> McpResult<String> {
        if self.job_manager.cancel(&job_id).await {
            Ok(format!("cancelled {job_id}"))
        } else {
            Ok(format!("not_cancellable {job_id}"))
        }
    }
}

pub async fn run_mcp_stdio() -> anyhow::Result<()> {
    let z3_bin = solver::find_or_download_z3().await?;

    let server = Z39Server {
        z3_bin: z3_bin.clone(),
        job_manager: Arc::new(job::JobManager::new(z3_bin)),
    };

    server.run_stdio().await?;
    Ok(())
}
