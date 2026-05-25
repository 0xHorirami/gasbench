use clap::Parser;
use eyre::Result;
use gasbench::{self, BenchConfig, FunctionEntry};
use std::fs;

/// GasBench — benchmark gas costs for smart contract functions
#[derive(Parser, Debug)]
#[command(name = "gasbench", version, about, long_about = None)]
struct Args {
    /// Contract address to benchmark
    #[arg(short, long)]
    contract: String,

    /// RPC endpoint URL
    #[arg(short, long, default_value = "http://localhost:8545")]
    rpc: String,

    /// Number of iterations per function
    #[arg(short, long, default_value_t = 5)]
    iterations: u32,

    /// Path to JSON file describing functions to benchmark
    #[arg(short, long)]
    functions_file: String,

    /// Output results as JSON
    #[arg(long)]
    json: bool,
}

/// Top-level JSON schema for the functions file
#[derive(serde::Deserialize)]
struct FunctionsFile {
    functions: Vec<FunctionEntry>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    println!("⛽ GasBench v{}", env!("CARGO_PKG_VERSION"));
    println!("   Contract : {}", args.contract);
    println!("   RPC      : {}", args.rpc);
    println!("   Iters    : {}", args.iterations);
    println!("   Functions: {}", args.functions_file);
    println!();

    // Load function definitions
    let file_content = fs::read_to_string(&args.functions_file)
        .unwrap_or_else(|e| {
            eprintln!("Failed to read functions file: {e}");
            std::process::exit(1);
        });
    let funcs_file: FunctionsFile = serde_json::from_str(&file_content)
        .unwrap_or_else(|e| {
            eprintln!("Failed to parse functions JSON: {e}");
            std::process::exit(1);
        });

    let config = BenchConfig::new(
        &args.contract,
        &args.rpc,
        args.iterations,
        funcs_file.functions,
    )?;

    println!("Running benchmarks...");
    let results = gasbench::run_benchmarks(&config).await?;

    let stats = gasbench::compute_stats(&results);
    let suggestions = gasbench::generate_suggestions(&stats);

    if args.json {
        println!("{}", serde_json::to_string_pretty(&stats)?);
    } else {
        gasbench::print_report(&stats, &suggestions);
    }

    Ok(())
}
