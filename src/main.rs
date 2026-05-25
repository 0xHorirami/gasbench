use clap::Parser;
use gasbench::{run_benchmark, BenchmarkConfig};

/// GasBench — Gas benchmarking tool for EVM smart contracts
#[derive(Parser, Debug)]
#[command(name = "gasbench", version, about, long_about = None)]
struct Cli {
    /// Contract address to benchmark
    #[arg(long)]
    contract: String,

    /// RPC endpoint URL
    #[arg(long, default_value = "http://localhost:8545")]
    rpc: String,

    /// Comma-separated list of function signatures to benchmark (e.g. "transfer(address,uint256),approve(address,uint256)")
    /// If omitted, all read/write functions from the ABI will be benchmarked.
    #[arg(long, value_delimiter = ',')]
    functions: Option<Vec<String>>,

    /// Number of iterations per function
    #[arg(long, default_value_t = 5)]
    iterations: usize,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let config = BenchmarkConfig {
        contract_address: cli.contract,
        rpc_url: cli.rpc,
        functions: cli.functions,
        iterations: cli.iterations,
    };

    let report = run_benchmark(config).await?;
    println!("{}", report);

    Ok(())
}
