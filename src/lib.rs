use anyhow::{anyhow, Result};
use ethers::abi::{Abi, ParamType, Token};
use ethers::prelude::*;
use serde::Serialize;
use std::str::FromStr;
use std::sync::Arc;
use tabled::{Table, Tabled};

/// Configuration for a benchmark run.
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub contract_address: String,
    pub rpc_url: String,
    pub functions: Option<Vec<String>>,
    pub iterations: usize,
}

/// Result of a single function call benchmark.
#[derive(Debug, Clone, Serialize)]
pub struct FunctionBenchmark {
    pub function_name: String,
    pub gas_used: Vec<u64>,
    pub avg_gas: u64,
    pub min_gas: u64,
    pub max_gas: u64,
    pub std_dev: f64,
    pub anomaly_detected: bool,
}

/// Snapshot of a function's info collected from the ABI before borrowing ends.
#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub name: String,
    pub input_types: Vec<ParamType>,
}

#[derive(Tabled)]
struct BenchmarkRow {
    #[tabled(rename = "Function")]
    function: String,
    #[tabled(rename = "Avg Gas")]
    avg_gas: String,
    #[tabled(rename = "Min")]
    min_gas: String,
    #[tabled(rename = "Max")]
    max_gas: String,
    #[tabled(rename = "Std Dev")]
    std_dev: String,
    #[tabled(rename = "Anomaly")]
    anomaly: String,
}

/// Generate default sample tokens for common EVM param types.
fn sample_tokens(param_types: &[ParamType]) -> Vec<Token> {
    param_types
        .iter()
        .map(sample_token_for_type)
        .collect()
}

fn sample_token_for_type(param_type: &ParamType) -> Token {
    match param_type {
        ParamType::Address => {
            Token::Address(Address::from_str("0x0000000000000000000000000000000000000001").unwrap())
        }
        ParamType::Uint(_bits) => Token::Uint(1u64.into()),
        ParamType::Int(_bits) => Token::Int(1i64.into()),
        ParamType::Bool => Token::Bool(true),
        ParamType::String => Token::String("benchmark".to_string()),
        ParamType::Bytes => Token::Bytes(vec![0u8; 32]),
        ParamType::FixedBytes(n) => {
            let mut b = vec![0u8; *n];
            if *n > 0 { b[0] = 1; }
            Token::FixedBytes(b)
        }
        ParamType::Array(inner) => {
            Token::Array(vec![sample_token_for_type(inner)])
        }
        ParamType::FixedArray(inner, size) => {
            let items: Vec<Token> = (0..*size).map(|_| sample_token_for_type(inner)).collect();
            Token::FixedArray(items)
        }
        ParamType::Tuple(fields) => {
            Token::Tuple(fields.iter().map(|f| sample_token_for_type(f)).collect())
        }
    }
}

/// Enumerate all functions from the ABI as owned FunctionInfo, optionally filtered by name.
pub fn get_function_infos(abi: &Abi, filter: &Option<Vec<String>>) -> Vec<FunctionInfo> {
    abi.functions()
        .map(|func| FunctionInfo {
            name: func.name.clone(),
            input_types: func.inputs.iter().map(|p| p.kind.clone()).collect(),
        })
        .filter(|info| {
            match filter {
                Some(names) => names.iter().any(|n| n == &info.name),
                None => true,
            }
        })
        .collect()
}

/// Compute mean of a slice.
pub fn mean(data: &[u64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    data.iter().sum::<u64>() as f64 / data.len() as f64
}

/// Compute standard deviation.
pub fn std_dev(data: &[u64]) -> f64 {
    if data.len() < 2 {
        return 0.0;
    }
    let avg = mean(data);
    let variance = data
        .iter()
        .map(|v| {
            let diff = *v as f64 - avg;
            diff * diff
        })
        .sum::<f64>()
        / data.len() as f64;
    variance.sqrt()
}

/// Detect anomalies: any value > 2 standard deviations from the mean.
pub fn detect_anomaly(data: &[u64]) -> bool {
    if data.len() < 3 {
        return false;
    }
    let avg = mean(data);
    let sd = std_dev(data);
    if sd == 0.0 {
        return false;
    }
    data.iter().any(|v| (*v as f64 - avg).abs() > 2.0 * sd)
}

/// Format the benchmark report as a table string.
fn format_report(results: &[FunctionBenchmark]) -> String {
    let rows: Vec<BenchmarkRow> = results
        .iter()
        .map(|r| BenchmarkRow {
            function: r.function_name.clone(),
            avg_gas: format!("{}", r.avg_gas),
            min_gas: format!("{}", r.min_gas),
            max_gas: format!("{}", r.max_gas),
            std_dev: format!("{:.2}", r.std_dev),
            anomaly: if r.anomaly_detected { "⚠ YES" } else { "✓ No" }.to_string(),
        })
        .collect();

    if rows.is_empty() {
        return "No functions benchmarked.".to_string();
    }

    let table = Table::new(rows).to_string();

    let mut report = String::new();
    report.push_str("\n=== GasBench Report ===\n\n");
    report.push_str(&table);
    report.push('\n');

    // Summary
    let total_avg: u64 = results.iter().map(|r| r.avg_gas).sum();
    let anomaly_count = results.iter().filter(|r| r.anomaly_detected).count();
    report.push_str(&format!(
        "\nFunctions benchmarked: {}\nTotal avg gas: {}\nAnomalies detected: {}\n",
        results.len(),
        total_avg,
        anomaly_count
    ));

    if anomaly_count > 0 {
        report.push_str("\n⚠ Optimization suggestions:\n");
        for r in results.iter().filter(|r| r.anomaly_detected) {
            report.push_str(&format!(
                "  - `{}`: high gas variance (std_dev={:.2}), consider optimizing storage writes or loop bounds.\n",
                r.function_name, r.std_dev
            ));
        }
    }

    report
}

/// Run the full benchmark pipeline.
pub async fn run_benchmark(config: BenchmarkConfig) -> Result<String> {
    let provider = Provider::<Http>::try_from(&config.rpc_url)
        .map_err(|e| anyhow!("Failed to connect to RPC at {}: {}", config.rpc_url, e))?;
    let client = Arc::new(provider);

    let address = Address::from_str(&config.contract_address)
        .map_err(|e| anyhow!("Invalid contract address '{}': {}", config.contract_address, e))?;

    // Load ABI. For a production tool you'd load from a file or explorer API.
    // Here we use a built-in minimal ERC-20 ABI for demonstration.
    let abi: Abi = serde_json::from_str(ERC20_ABI_JSON)
        .map_err(|e| anyhow!("Failed to parse built-in ABI: {}", e))?;

    // Collect function info before moving abi into the contract
    let function_infos = get_function_infos(&abi, &config.functions);

    if function_infos.is_empty() {
        return Err(anyhow!("No matching functions found in ABI."));
    }

    let contract = Contract::new(address, abi, client.clone());
    let mut results: Vec<FunctionBenchmark> = Vec::new();

    for info in &function_infos {
        let sample_args = sample_tokens(&info.input_types);
        let mut gas_values: Vec<u64> = Vec::new();

        for _ in 0..config.iterations {
            let call = contract.method::<_, ()>(&info.name, sample_args.clone());
            match call {
                Ok(pending) => {
                    match pending.estimate_gas().await {
                        Ok(gas) => {
                            gas_values.push(gas.as_u64());
                        }
                        Err(e) => {
                            eprintln!(
                                "Warning: gas estimation failed for `{}`: {} — recording 0",
                                info.name, e
                            );
                            gas_values.push(0);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Warning: could not build call for `{}`: {}", info.name, e);
                    gas_values.push(0);
                }
            }
        }

        let anomaly = detect_anomaly(&gas_values);
        let avg = mean(&gas_values) as u64;
        let min = *gas_values.iter().min().unwrap_or(&0);
        let max = *gas_values.iter().max().unwrap_or(&0);
        let sd = std_dev(&gas_values);

        results.push(FunctionBenchmark {
            function_name: info.name.clone(),
            gas_used: gas_values,
            avg_gas: avg,
            min_gas: min,
            max_gas: max,
            std_dev: sd,
            anomaly_detected: anomaly,
        });
    }

    Ok(format_report(&results))
}

/// Minimal ERC-20 ABI used when no external ABI file is provided.
const ERC20_ABI_JSON: &str = r#"[
    {
        "type": "function",
        "name": "totalSupply",
        "inputs": [],
        "outputs": [{"name":"","type":"uint256"}],
        "stateMutability": "view"
    },
    {
        "type": "function",
        "name": "balanceOf",
        "inputs": [{"name":"account","type":"address"}],
        "outputs": [{"name":"","type":"uint256"}],
        "stateMutability": "view"
    },
    {
        "type": "function",
        "name": "transfer",
        "inputs": [{"name":"recipient","type":"address"},{"name":"amount","type":"uint256"}],
        "outputs": [{"name":"","type":"bool"}],
        "stateMutability": "nonpayable"
    },
    {
        "type": "function",
        "name": "allowance",
        "inputs": [{"name":"owner","type":"address"},{"name":"spender","type":"address"}],
        "outputs": [{"name":"","type":"uint256"}],
        "stateMutability": "view"
    },
    {
        "type": "function",
        "name": "approve",
        "inputs": [{"name":"spender","type":"address"},{"name":"amount","type":"uint256"}],
        "outputs": [{"name":"","type":"bool"}],
        "stateMutability": "nonpayable"
    },
    {
        "type": "function",
        "name": "transferFrom",
        "inputs": [{"name":"sender","type":"address"},{"name":"recipient","type":"address"},{"name":"amount","type":"uint256"}],
        "outputs": [{"name":"","type":"bool"}],
        "stateMutability": "nonpayable"
    },
    {
        "type": "function",
        "name": "name",
        "inputs": [],
        "outputs": [{"name":"","type":"string"}],
        "stateMutability": "view"
    },
    {
        "type": "function",
        "name": "symbol",
        "inputs": [],
        "outputs": [{"name":"","type":"string"}],
        "stateMutability": "view"
    },
    {
        "type": "function",
        "name": "decimals",
        "inputs": [],
        "outputs": [{"name":"","type":"uint8"}],
        "stateMutability": "view"
    }
]"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mean_basic() {
        let data = vec![100, 200, 300];
        let m = mean(&data);
        assert!((m - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_mean_empty() {
        let data: Vec<u64> = vec![];
        assert_eq!(mean(&data), 0.0);
    }

    #[test]
    fn test_std_dev_zero() {
        let data = vec![500, 500, 500];
        assert!((std_dev(&data) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_std_dev_nonzero() {
        let data = vec![100, 200, 300];
        let sd = std_dev(&data);
        assert!(sd > 80.0 && sd < 83.0); // ~81.65
    }

    #[test]
    fn test_detect_anomaly_false_for_consistent_data() {
        let data = vec![21000, 21000, 21000, 21000, 21000];
        assert!(!detect_anomaly(&data));
    }

    #[test]
    fn test_detect_anomaly_true_for_spike() {
        let data = vec![21000, 21000, 21000, 21000, 21000, 21000, 21000, 21000, 21000, 500000];
        assert!(detect_anomaly(&data));
    }

    #[test]
    fn test_detect_anomaly_insufficient_data() {
        let data = vec![21000, 210000];
        assert!(!detect_anomaly(&data)); // less than 3 data points
    }

    #[test]
    fn test_sample_token_address() {
        let token = sample_token_for_type(&ParamType::Address);
        match token {
            Token::Address(_) => {}
            _ => panic!("Expected Address token"),
        }
    }

    #[test]
    fn test_sample_token_uint() {
        let token = sample_token_for_type(&ParamType::Uint(256));
        match token {
            Token::Uint(v) => assert_eq!(v.as_u64(), 1),
            _ => panic!("Expected Uint token"),
        }
    }

    #[test]
    fn test_get_function_infos_with_filter() {
        let abi: Abi = serde_json::from_str(ERC20_ABI_JSON).unwrap();
        let filter = Some(vec!["transfer".to_string()]);
        let funcs = get_function_infos(&abi, &filter);
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "transfer");
    }

    #[test]
    fn test_get_function_infos_no_filter() {
        let abi: Abi = serde_json::from_str(ERC20_ABI_JSON).unwrap();
        let funcs = get_function_infos(&abi, &None);
        assert!(funcs.len() >= 9); // ERC-20 has 9 functions
    }
}
