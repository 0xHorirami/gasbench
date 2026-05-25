# GasBench ⛽

A command-line gas benchmarking tool for EVM smart contracts. GasBench connects to any EVM-compatible RPC endpoint, enumerates contract functions, measures gas consumption across multiple iterations, and detects anomalies to help you optimize your smart contracts.

## Features

- **Multi-function benchmarking** — Automatically discover and benchmark all functions from a contract's ABI
- **Iteration-based analysis** — Run each function multiple times and compute statistical metrics
- **Anomaly detection** — Flag functions with high gas variance (>2σ from mean) that may need optimization
- **Formatted reports** — Clean tabular output with avg, min, max, std dev, and anomaly flags
- **Configurable** — Filter specific functions or tune the iteration count

## Installation

```bash
# Clone the repo
git clone https://github.com/0xHorirami/gasbench.git
cd gasbench

# Build
cargo build --release

# Binary will be at target/release/gasbench
```

## Usage

```bash
# Benchmark all ERC-20 functions on a contract
gasbench \
  --contract 0x1234567890abcdef1234567890abcdef12345678 \
  --rpc https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY \
  --iterations 10

# Benchmark only specific functions
gasbench \
  --contract 0x1234567890abcdef1234567890abcdef12345678 \
  --rpc https://rpc.ankr.com/eth \
  --functions transfer,approve,balanceOf \
  --iterations 5

# Use a local node
gasbench \
  --contract 0x5FbDB2315678afecb367f032d93F642f64180aa3 \
  --rpc http://localhost:8545
```

### CLI Options

| Flag | Description | Default |
|------|-------------|---------|
| `--contract` | Contract address to benchmark | *required* |
| `--rpc` | JSON-RPC endpoint URL | `http://localhost:8545` |
| `--functions` | Comma-separated function names to benchmark | All functions |
| `--iterations` | Number of gas estimation iterations per function | `5` |

## Example Output

```
=== GasBench Report ===

+---------------+----------+-------+--------+---------+---------+
| Function      | Avg Gas  | Min   | Max    | Std Dev | Anomaly |
+---------------+----------+-------+--------+---------+---------+
| totalSupply   | 2350     | 2350  | 2350   | 0.00    | ✓ No    |
| balanceOf     | 2650     | 2650  | 2650   | 0.00    | ✓ No    |
| transfer      | 51234    | 51234 | 51234  | 0.00    | ✓ No    |
| approve       | 46123    | 46123 | 46200  | 31.40   | ✓ No    |
+---------------+----------+-------+--------+---------+---------+

Functions benchmarked: 4
Total avg gas: 102357
Anomalies detected: 0
```

## How It Works

1. **Connect** — Establishes an HTTP connection to the specified RPC endpoint
2. **ABI Discovery** — Uses a built-in ABI (or you can extend it for your contract)
3. **Function Enumeration** — Optionally filters the function list by name
4. **Gas Estimation** — Calls `estimate_gas()` on each function with sample parameters, repeated `N` iterations
5. **Statistical Analysis** — Computes mean, min, max, and standard deviation per function
6. **Anomaly Detection** — Flags any iteration where gas exceeds 2σ from the mean (suggesting state-dependent cost variance)
7. **Report** — Prints a formatted table with optimization suggestions

## Tech Stack

- **[ethers-rs](https://github.com/gakonst/ethers-rs)** — Ethereum interaction
- **[clap](https://github.com/clap-rs/clap)** — CLI argument parsing
- **[tokio](https://tokio.rs/)** — Async runtime
- **[tabled](https://github.com/zhiburt/tabled)** — Table formatting
- **[serde](https://serde.rs/)** — JSON serialization

## Running Tests

```bash
cargo test
```

## License

MIT
