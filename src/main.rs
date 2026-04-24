use std::io::Read;
use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};

use z39::{domains, solver};

#[derive(Parser)]
#[command(
    name = "z39",
    version,
    about = "Z3-powered reasoning for AI agents — scheduling, logic, config, safety. Single binary: CLI by default, MCP server with `z39 mcp`."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check if a schedule is feasible. Input JSON: {tasks:[{name,duration}],slot_start,slot_end,constraints}
    Schedule {
        /// JSON payload. Use "-" to read from stdin, or omit and pass --file
        input: Option<String>,
        /// Read JSON from file
        #[arg(long, short = 'f')]
        file: Option<PathBuf>,
        /// Solver timeout in seconds
        #[arg(long, default_value = "30")]
        timeout: u64,
    },
    /// Verify boolean logic. Input JSON: {description,check:{type,vars,condition/expr_a/expr_b/rules/conditions}}
    Logic {
        input: Option<String>,
        #[arg(long, short = 'f')]
        file: Option<PathBuf>,
        #[arg(long, default_value = "30")]
        timeout: u64,
    },
    /// Validate configuration constraints. Input JSON: {vars:[{name,var_type,allowed_values}],rules,mode}
    Config {
        input: Option<String>,
        #[arg(long, short = 'f')]
        file: Option<PathBuf>,
        #[arg(long, default_value = "15")]
        timeout: u64,
    },
    /// Pre-check an action against safety rules. Input JSON: {action:{kind,target,destructive},protected,rules}
    Safety {
        input: Option<String>,
        #[arg(long, short = 'f')]
        file: Option<PathBuf>,
    },
    /// Send raw SMT-LIB2 to Z3
    Solve {
        /// SMT-LIB2 formula. Use "-" for stdin, or omit and pass --file
        formula: Option<String>,
        #[arg(long, short = 'f')]
        file: Option<PathBuf>,
        #[arg(long, default_value = "30")]
        timeout: u64,
    },
    /// Start MCP server (STDIO transport)
    Mcp,
}

fn read_input(positional: Option<String>, file: Option<PathBuf>) -> anyhow::Result<String> {
    match (positional, file) {
        (Some(_), Some(_)) => {
            anyhow::bail!("provide either a positional argument or --file, not both")
        }
        (None, None) => anyhow::bail!(
            "missing input: pass the payload as an argument, use '-' for stdin, or pass --file <path>"
        ),
        (_, Some(path)) => std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display())),
        (Some(s), None) if s == "-" => {
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .context("reading stdin")?;
            Ok(buf)
        }
        (Some(s), None) => Ok(s),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Mcp => z39::mcp::run_mcp_stdio().await?,

        Command::Schedule { input, file, timeout } => {
            let payload = read_input(input, file)?;
            let z3_bin = solver::find_or_download_z3().await?;
            let out = domains::schedule::run(&z3_bin, &payload, timeout)
                .await
                .context("invalid schedule JSON")?;
            println!("{out}");
        }

        Command::Logic { input, file, timeout } => {
            let payload = read_input(input, file)?;
            let z3_bin = solver::find_or_download_z3().await?;
            let out = domains::logic::run(&z3_bin, &payload, timeout)
                .await
                .context("invalid logic JSON")?;
            println!("{out}");
        }

        Command::Config { input, file, timeout } => {
            let payload = read_input(input, file)?;
            let z3_bin = solver::find_or_download_z3().await?;
            let out = domains::config::run(&z3_bin, &payload, timeout)
                .await
                .context("invalid config JSON")?;
            println!("{out}");
        }

        Command::Safety { input, file } => {
            let payload = read_input(input, file)?;
            let out = domains::safety::run(&payload).context("invalid safety JSON")?;
            println!("{out}");
        }

        Command::Solve { formula, file, timeout } => {
            let payload = read_input(formula, file)?;
            let z3_bin = solver::find_or_download_z3().await?;
            let result = solver::solve(&z3_bin, &payload, timeout).await;
            println!("{}", result.to_compact());
        }
    }

    Ok(())
}
