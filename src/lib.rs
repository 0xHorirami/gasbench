use ethers::prelude::*;
use eyre::{Result, WrapErr};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tabled::{Table, Tabled};

/// Result of a single gas benchmark run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub function_name: String,
    pub iteration: u32,
    pub gas_used: u64,
    pub success: bool,
    pub error: Option<String>,
}

/// Aggregated stats for a function across all iterations
#[derive(Debug, Clone, Serialize, Deserialize, Tabled)]
pub struct FunctionStats {
    pub function_name: String,
    pub iterations: u32,
    pub min_gas: u64,
    pub max_gas: u64,
    pub avg_gas: f64,
    pub median_gas: u64,
    pub std_dev: f64,
}

/// Optimization suggestion for a function
#[derive(Debug, Clone, Tabled)]
pub struct OptimizationSuggestion {
    pub function_name: String,
    pub severity: String,
    pub suggestion: String,
}

/// ABI function parameter type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionParam {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: String,
    pub value: serde_json::Value,
}

/// ABI function entry for benchmarking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionEntry {
    pub name: String,
    pub inputs: Vec<FunctionParam>,
    #[serde(default)]
    pub state_mutability: String,
}

/// Benchmark configuration
#[derive(Debug, Clone)]
pub struct BenchConfig {
    pub contract_address: Address,
    pub rpc_url: String,
    pub iterations: u32,
    pub functions: Vec<FunctionEntry>,
}

impl BenchConfig {
    pub fn new(
        contract_address: &str,
        rpc_url: &str,
        iterations: u32,
        functions: Vec<FunctionEntry>,
    ) -> Result<Self> {
        Ok(Self {
            contract_address: Address::from_str(contract_address)
                .wrap_err("Invalid contract address")?,
            rpc_url: rpc_url.to_string(),
            iterations,
            functions,
        })
    }
}

/// Run benchmarks for all configured functions
pub async fn run_benchmarks(config: &BenchConfig) -> Result<Vec<BenchmarkResult>> {
    let provider = Provider::<Http>::try_from(&config.rpc_url)
        .wrap_err("Failed to connect to RPC endpoint")?;

    let mut all_results = Vec::new();

    for func in &config.functions {
        for iteration in 0..config.iterations {
            let result = benchmark_function(&provider, config, func, iteration).await;
            all_results.push(result);
        }
    }

    Ok(all_results)
}

/// Benchmark a single function call
async fn benchmark_function(
    provider: &Provider<Http>,
    config: &BenchConfig,
    func: &FunctionEntry,
    iteration: u32,
) -> BenchmarkResult {
    let call_result = async {
        // Build function ABI
        let inputs: Vec<(String, ethers::abi::ParamType)> = func
            .inputs
            .iter()
            .map(|p| {
                let pt = parse_param_type(&p.param_type)?;
                Ok((p.name.clone(), pt))
            })
            .collect::<Result<Vec<_>>>()?;

        let func_abi = ethers::abi::Function {
            name: func.name.clone(),
            inputs: inputs
                .iter()
                .map(|(name, pt)| ethers::abi::Param {
                    name: name.clone(),
                    kind: pt.clone(),
                    internal_type: None,
                })
                .collect(),
            outputs: vec![],
            constant: false,
            state_mutability: ethers::abi::StateMutability::NonPayable,
        };

        // Encode function call
        let tokens: Vec<ethers::abi::Token> = func
            .inputs
            .iter()
            .zip(func_abi.inputs.iter())
            .map(|(param, abi_param)| json_to_token(&param.value, &abi_param.kind))
            .collect::<Result<Vec<_>>>()?;

        let data = func_abi.encode_input(&tokens)?;

        let tx = TransactionRequest::new()
            .to(config.contract_address)
            .data(data);

        let gas = provider
            .estimate_gas(&tx.into(), None)
            .await
            .wrap_err("Gas estimation failed")?;

        Ok::<u64, eyre::Report>(gas.as_u64())
    }
    .await;

    match call_result {
        Ok(gas_used) => BenchmarkResult {
            function_name: func.name.clone(),
            iteration,
            gas_used,
            success: true,
            error: None,
        },
        Err(e) => BenchmarkResult {
            function_name: func.name.clone(),
            iteration,
            gas_used: 0,
            success: false,
            error: Some(e.to_string()),
        },
    }
}

/// Parse a Solidity type string into ParamType
pub fn parse_param_type(ty: &str) -> Result<ethers::abi::ParamType> {
    match ty {
        "address" => Ok(ethers::abi::ParamType::Address),
        "bool" => Ok(ethers::abi::ParamType::Bool),
        "string" => Ok(ethers::abi::ParamType::String),
        "bytes" => Ok(ethers::abi::ParamType::Bytes),
        "uint256" | "uint" => Ok(ethers::abi::ParamType::Uint(256)),
        "int256" | "int" => Ok(ethers::abi::ParamType::Int(256)),
        "uint128" => Ok(ethers::abi::ParamType::Uint(128)),
        "uint64" => Ok(ethers::abi::ParamType::Uint(64)),
        "uint32" => Ok(ethers::abi::ParamType::Uint(32)),
        "uint8" => Ok(ethers::abi::ParamType::Uint(8)),
        "int128" => Ok(ethers::abi::ParamType::Int(128)),
        "int64" => Ok(ethers::abi::ParamType::Int(64)),
        "int32" => Ok(ethers::abi::ParamType::Int(32)),
        "int8" => Ok(ethers::abi::ParamType::Int(8)),
        "bytes32" => Ok(ethers::abi::ParamType::FixedBytes(32)),
        "bytes1" => Ok(ethers::abi::ParamType::FixedBytes(1)),
        ty if ty.starts_with("uint") && ty.ends_with(']') => {
            Ok(ethers::abi::ParamType::Uint(256)) // simplified
        }
        _ => eyre::bail!("Unsupported param type: {}", ty),
    }
}

/// Convert a JSON value to an ABI token
pub fn json_to_token(
    value: &serde_json::Value,
    param_type: &ethers::abi::ParamType,
) -> Result<ethers::abi::Token> {
    use ethers::abi::{ParamType, Token};
    match param_type {
        ParamType::Address => {
            let s = value.as_str().ok_or_else(|| eyre::eyre!("Expected string for address"))?;
            Ok(Token::Address(Address::from_str(s)?))
        }
        ParamType::Uint(_) => {
            if let Some(n) = value.as_u64() {
                Ok(Token::Uint(U256::from(n)))
            } else if let Some(s) = value.as_str() {
                Ok(Token::Uint(U256::from_dec_str(s)?))
            } else {
                eyre::bail!("Expected number or string for uint")
            }
        }
        ParamType::Int(_) => {
            if let Some(n) = value.as_i64() {
                Ok(Token::Int(I256::from(n).into()))
            } else if let Some(s) = value.as_str() {
                Ok(Token::Int(I256::from_dec_str(s)?.into()))
            } else {
                eyre::bail!("Expected number or string for int")
            }
        }
        ParamType::Bool => {
            let b = value.as_bool().ok_or_else(|| eyre::eyre!("Expected bool"))?;
            Ok(Token::Bool(b))
        }
        ParamType::String => {
            let s = value.as_str().ok_or_else(|| eyre::eyre!("Expected string"))?;
            Ok(Token::String(s.to_string()))
        }
        ParamType::Bytes => {
            let s = value.as_str().ok_or_else(|| eyre::eyre!("Expected hex string for bytes"))?;
            let bytes = hex::decode(s.strip_prefix("0x").unwrap_or(s))?;
            Ok(Token::Bytes(bytes))
        }
        ParamType::FixedBytes(n) => {
            let s = value.as_str().ok_or_else(|| eyre::eyre!("Expected hex string for bytesN"))?;
            let bytes = hex::decode(s.strip_prefix("0x").unwrap_or(s))?;
            let mut fixed = [0u8; 32];
            let len = bytes.len().min(*n).min(32);
            fixed[..len].copy_from_slice(&bytes[..len]);
            Ok(Token::FixedBytes(fixed.to_vec()))
        }
        _ => eyre::bail!("Unsupported param type for token conversion"),
    }
}

/// Compute statistics for benchmark results
pub fn compute_stats(results: &[BenchmarkResult]) -> Vec<FunctionStats> {
    use std::collections::HashMap;
    let mut groups: HashMap<String, Vec<u64>> = HashMap::new();

    for r in results {
        if r.success {
            groups.entry(r.function_name.clone()).or_default().push(r.gas_used);
        }
    }

    let mut stats = Vec::new();
    for (name, mut values) in groups {
        values.sort_unstable();
        let count = values.len() as u32;
        let min = *values.first().unwrap_or(&0);
        let max = *values.last().unwrap_or(&0);
        let sum: u64 = values.iter().sum();
        let avg = sum as f64 / count as f64;
        let median = values[count as usize / 2];

        let variance = values.iter().map(|&v| {
            let diff = v as f64 - avg;
            diff * diff
        }).sum::<f64>() / count as f64;
        let std_dev = variance.sqrt();

        stats.push(FunctionStats {
            function_name: name,
            iterations: count,
            min_gas: min,
            max_gas: max,
            avg_gas: avg,
            median_gas: median,
            std_dev,
        });
    }

    stats.sort_by(|a, b| b.avg_gas.partial_cmp(&a.avg_gas).unwrap_or(std::cmp::Ordering::Equal));
    stats
}

/// Generate optimization suggestions based on stats
pub fn generate_suggestions(stats: &[FunctionStats]) -> Vec<OptimizationSuggestion> {
    let mut suggestions = Vec::new();

    for s in stats {
        // High variance suggests inconsistent execution paths
        if s.std_dev > s.avg_gas * 0.2 && s.iterations > 2 {
            suggestions.push(OptimizationSuggestion {
                function_name: s.function_name.clone(),
                severity: "WARN".to_string(),
                suggestion: format!(
                    "High gas variance ({:.1} std dev vs {:.1} avg). Consider using require() \
                     guards earlier to fail fast, or review conditional branching.",
                    s.std_dev, s.avg_gas
                ),
            });
        }

        // Very high gas usage
        if s.avg_gas > 500_000.0 {
            suggestions.push(OptimizationSuggestion {
                function_name: s.function_name.clone(),
                severity: "HIGH".to_string(),
                suggestion: "Gas exceeds 500k. Consider: using mappings over arrays, \
                    batching operations, or splitting into multiple transactions."
                    .to_string(),
            });
        } else if s.avg_gas > 200_000.0 {
            suggestions.push(OptimizationSuggestion {
                function_name: s.function_name.clone(),
                severity: "MED".to_string(),
                suggestion: "Gas exceeds 200k. Consider: packing storage variables, \
                    using unchecked blocks for safe arithmetic, or caching storage reads."
                    .to_string(),
            });
        }

        // General suggestions for any function
        if s.avg_gas > 50_000.0 && suggestions.iter().filter(|sg| sg.function_name == s.function_name).count() == 0 {
            suggestions.push(OptimizationSuggestion {
                function_name: s.function_name.clone(),
                severity: "INFO".to_string(),
                suggestion: "Consider using calldata instead of memory for read-only \
                    function parameters to save gas.".to_string(),
            });
        }
    }

    if suggestions.is_empty() {
        suggestions.push(OptimizationSuggestion {
            function_name: "*".to_string(),
            severity: "OK".to_string(),
            suggestion: "All functions appear reasonably gas-efficient.".to_string(),
        });
    }

    suggestions
}

/// Print a formatted benchmark report
pub fn print_report(stats: &[FunctionStats], suggestions: &[OptimizationSuggestion]) {
    use colored::Colorize;

    println!("\n{}", "╔══════════════════════════════════════════════════════════════╗".cyan());
    println!("{}", "║               ⛽  GASBENCH REPORT                           ║".cyan());
    println!("{}", "╚══════════════════════════════════════════════════════════════╝".cyan());

    println!("\n{}", "── Gas Statistics ──────────────────────────────────────────────".bold());
    if stats.is_empty() {
        println!("  No successful benchmark results.");
    } else {
        let table = Table::new(stats).to_string();
        println!("{}", table);
    }

    println!("\n{}", "── Optimization Suggestions ───────────────────────────────────".bold());
    if suggestions.is_empty() {
        println!("  No suggestions.");
    } else {
        let table = Table::new(suggestions).to_string();
        println!("{}", table);
    }

    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_param_type_common() {
        assert!(matches!(parse_param_type("address").unwrap(), ethers::abi::ParamType::Address));
        assert!(matches!(parse_param_type("bool").unwrap(), ethers::abi::ParamType::Bool));
        assert!(matches!(parse_param_type("string").unwrap(), ethers::abi::ParamType::String));
        assert!(matches!(parse_param_type("uint256").unwrap(), ethers::abi::ParamType::Uint(256)));
        assert!(matches!(parse_param_type("int256").unwrap(), ethers::abi::ParamType::Int(256)));
        assert!(matches!(parse_param_type("bytes32").unwrap(), ethers::abi::ParamType::FixedBytes(32)));
        assert!(parse_param_type("totally_invalid").is_err());
    }

    #[test]
    fn test_json_to_token_address() {
        let addr = "0x0000000000000000000000000000000000000001";
        let token = json_to_token(&json!(addr), &ethers::abi::ParamType::Address).unwrap();
        match token {
            ethers::abi::Token::Address(a) => assert_eq!(a, Address::from_str(addr).unwrap()),
            _ => panic!("Expected Address token"),
        }
    }

    #[test]
    fn test_json_to_token_uint() {
        let token = json_to_token(&json!(42), &ethers::abi::ParamType::Uint(256)).unwrap();
        match token {
            ethers::abi::Token::Uint(n) => assert_eq!(n, U256::from(42)),
            _ => panic!("Expected Uint token"),
        }
    }

    #[test]
    fn test_json_to_token_string() {
        let token = json_to_token(&json!("hello"), &ethers::abi::ParamType::String).unwrap();
        match token {
            ethers::abi::Token::String(s) => assert_eq!(s, "hello"),
            _ => panic!("Expected String token"),
        }
    }

    #[test]
    fn test_compute_stats() {
        let results = vec![
            BenchmarkResult {
                function_name: "transfer".to_string(),
                iteration: 0,
                gas_used: 21000,
                success: true,
                error: None,
            },
            BenchmarkResult {
                function_name: "transfer".to_string(),
                iteration: 1,
                gas_used: 21000,
                success: true,
                error: None,
            },
            BenchmarkResult {
                function_name: "transfer".to_string(),
                iteration: 2,
                gas_used: 0,
                success: false,
                error: Some("fail".to_string()),
            },
        ];

        let stats = compute_stats(&results);
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].function_name, "transfer");
        assert_eq!(stats[0].iterations, 2);
        assert_eq!(stats[0].min_gas, 21000);
        assert_eq!(stats[0].max_gas, 21000);
        assert_eq!(stats[0].avg_gas, 21000.0);
    }

    #[test]
    fn test_generate_suggestions_high_gas() {
        let stats = vec![FunctionStats {
            function_name: "expensiveFunc".to_string(),
            iterations: 5,
            min_gas: 500_000,
            max_gas: 600_000,
            avg_gas: 550_000.0,
            median_gas: 550_000,
            std_dev: 10_000.0,
        }];

        let suggestions = generate_suggestions(&stats);
        assert!(!suggestions.is_empty());
        assert!(suggestions.iter().any(|s| s.severity == "HIGH"));
    }

    #[test]
    fn test_generate_suggestions_low_gas() {
        let stats = vec![FunctionStats {
            function_name: "cheapFunc".to_string(),
            iterations: 5,
            min_gas: 5000,
            max_gas: 5100,
            avg_gas: 5050.0,
            median_gas: 5050,
            std_dev: 40.0,
        }];

        let suggestions = generate_suggestions(&stats);
        assert!(suggestions.iter().any(|s| s.severity == "OK"));
    }

    #[test]
    fn test_bench_config_new_invalid_address() {
        let result = BenchConfig::new("not_an_address", "http://localhost:8545", 3, vec![]);
        assert!(result.is_err());
    }
}
