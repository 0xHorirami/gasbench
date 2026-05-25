# ⛽ GasBench

A Rust CLI tool for benchmarking gas costs of smart contract functions. Call each function with sample parameters, measure gas usage across multiple iterations, and receive an optimization report with actionable suggestions.

## Installation

```bash
cargo install --path .
```

## Usage

```bash
gasbench \
  --contract 0xYourContractAddress \
  --rpc https://mainnet.infura.io/v3/YOUR_KEY \
  --iterations 10 \
  --functions-file functions.json
```

### Functions JSON Format

Create a `functions.json` file describing the functions to benchmark:

```json
{
  "functions": [
    {
      "name": "transfer",
      "state_mutability": "nonpayable",
      "inputs": [
        { "name": "to", "type": "address", "value": "0x0000000000000000000000000000000000000001" },
        { "name": "amount", "type": "uint256", "value": "1000000000000000000" }
      ]
    }
  ]
}
```

### JSON Output

```bash
gasbench --contract 0x... --rpc http://localhost:8545 -f functions.json --json
```

## Example Output

```
╔══════════════════════════════════════════════════════════════╗
║               ⛽  GASBENCH REPORT                           ║
╚══════════════════════════════════════════════════════════════╝

── Gas Statistics ──────────────────────────────────────────────
 function_name | iterations | min_gas | max_gas | avg_gas  | median_gas | std_dev
 transfer      | 10         | 21000   | 21500   | 21200.0  | 21100      | 180.3
 balanceOf     | 10         | 2300    | 2300    | 2300.0   | 2300       | 0.0

── Optimization Suggestions ───────────────────────────────────
 function_name | severity | suggestion
 *             | OK       | All functions appear reasonably gas-efficient.
```

## Options

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--contract` | `-c` | required | Contract address |
| `--rpc` | `-r` | `http://localhost:8545` | RPC endpoint URL |
| `--iterations` | `-i` | `5` | Benchmark iterations per function |
| `--functions-file` | `-f` | required | Path to functions JSON |
| `--json` | — | false | Output raw JSON stats |

## Running Tests

```bash
cargo test
```

## License

MIT
