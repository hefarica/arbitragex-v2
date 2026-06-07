//! Universal Cartridge Contract — validation rules.
//!
//! Every `.rhai` script MUST export these functions to be considered a valid
//! cartridge. The contract enforcer validates the compiled AST before the
//! cartridge is admitted into the active registry.
//!
//! ## Required Functions
//!
//! | Signature                                    | Returns         |
//! |----------------------------------------------|-----------------|
//! | `fn init_strategy() -> Map`                  | Metadata map    |
//! | `fn evaluate_opportunity(pool_data) -> Map`  | Eval result map |
//! | `fn build_payload(opportunity) -> Map`       | Tx payload map  |
//!
//! ## Optional Functions (lifecycle hooks)
//!
//! | Signature                          | Purpose                          |
//! |------------------------------------|----------------------------------|
//! | `fn on_activate()`                 | Called when cartridge goes live   |
//! | `fn on_deactivate()`               | Called when cartridge is paused   |
//! | `fn on_new_block(block_number)`     | Called on each new block          |

use rhai::AST;
use std::collections::HashSet;

/// The three mandatory function names every cartridge must define.
pub const REQUIRED_FUNCTIONS: &[&str] = &[
    "init_strategy",
    "evaluate_opportunity",
    "build_payload",
];

/// Optional lifecycle hook functions.
pub const OPTIONAL_HOOKS: &[&str] = &[
    "on_activate",
    "on_deactivate",
    "on_new_block",
];

/// Metadata fields that `init_strategy()` MUST return in its Map.
pub const REQUIRED_METADATA_KEYS: &[&str] = &[
    "name",
    "version",
    "author",
    "description",
];

/// Validation errors for cartridge contract compliance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractViolation {
    /// A required function is missing from the AST.
    MissingFunction(String),
    /// A required function has wrong arity (expected, actual).
    WrongArity { function: String, expected: usize, actual: usize },
    /// The `init_strategy()` return map is missing a required key.
    MissingMetadataKey(String),
    /// The script contains forbidden operations (e.g., file I/O).
    ForbiddenOperation(String),
}

impl std::fmt::Display for ContractViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingFunction(name) => {
                write!(f, "missing required function: `fn {name}(..)`")
            }
            Self::WrongArity { function, expected, actual } => {
                write!(f, "function `{function}` expects {expected} params, found {actual}")
            }
            Self::MissingMetadataKey(key) => {
                write!(f, "init_strategy() return map missing required key: `{key}`")
            }
            Self::ForbiddenOperation(op) => {
                write!(f, "forbidden operation detected: {op}")
            }
        }
    }
}

impl std::error::Error for ContractViolation {}

/// Validates a compiled AST against the Universal Cartridge Contract.
///
/// Returns `Ok(())` if the cartridge is compliant, or a list of violations.
pub fn validate_contract(ast: &AST) -> Result<(), Vec<ContractViolation>> {
    let mut violations = Vec::new();

    // Collect all function names defined in the AST.
    let defined_functions: HashSet<String> = ast
        .iter_functions()
        .map(|f| f.name.to_string())
        .collect();

    // Check required functions exist.
    for &required in REQUIRED_FUNCTIONS {
        if !defined_functions.contains(required) {
            violations.push(ContractViolation::MissingFunction(required.to_owned()));
        }
    }

    // Check arity of required functions.
    for func_def in ast.iter_functions() {
        let expected_arity = match func_def.name.to_string().as_str() {
            "init_strategy" => Some(0),
            "evaluate_opportunity" => Some(1),
            "build_payload" => Some(1),
            _ => None,
        };
        if let Some(expected) = expected_arity {
            if func_def.params.len() != expected {
                violations.push(ContractViolation::WrongArity {
                    function: func_def.name.to_string(),
                    expected,
                    actual: func_def.params.len(),
                });
            }
        }
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

/// Validates the metadata map returned by `init_strategy()` at runtime.
///
/// Called after executing `init_strategy()` to ensure the returned map
/// contains all required keys with non-empty string values.
pub fn validate_metadata(map: &rhai::Map) -> Result<(), Vec<ContractViolation>> {
    let mut violations = Vec::new();

    for &key in REQUIRED_METADATA_KEYS {
        match map.get(key) {
            None => {
                violations.push(ContractViolation::MissingMetadataKey(key.to_owned()));
            }
            Some(val) => {
                if let Ok(s) = val.clone().into_string() {
                    if s.trim().is_empty() {
                        violations.push(ContractViolation::MissingMetadataKey(
                            format!("{key} (empty string)"),
                        ));
                    }
                }
            }
        }
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use rhai::Engine;

    fn make_engine() -> Engine {
        Engine::new()
    }

    #[test]
    fn valid_contract_passes() {
        let engine = make_engine();
        let ast = engine.compile(r#"
            fn init_strategy() {
                #{
                    name: "Test Strategy",
                    version: "1.0.0",
                    author: "test",
                    description: "A test cartridge"
                }
            }
            fn evaluate_opportunity(pool_data) {
                #{ is_opportunity: false, estimated_profit: 0.0, confidence: 0.0 }
            }
            fn build_payload(opportunity) {
                #{ target_contract: "0x0", calldata: "0x0", value_wei: "0", gas_limit: 21000 }
            }
        "#).unwrap();

        assert!(validate_contract(&ast).is_ok());
    }

    #[test]
    fn missing_function_fails() {
        let engine = make_engine();
        let ast = engine.compile(r#"
            fn init_strategy() { #{} }
            fn evaluate_opportunity(pool_data) { #{} }
            // build_payload is missing
        "#).unwrap();

        let result = validate_contract(&ast);
        assert!(result.is_err());
        let violations = result.unwrap_err();
        assert!(violations.iter().any(|v| matches!(v, ContractViolation::MissingFunction(name) if name == "build_payload")));
    }

    #[test]
    fn wrong_arity_fails() {
        let engine = make_engine();
        let ast = engine.compile(r#"
            fn init_strategy(unexpected_param) { #{} }
            fn evaluate_opportunity(pool_data) { #{} }
            fn build_payload(opportunity) { #{} }
        "#).unwrap();

        let result = validate_contract(&ast);
        assert!(result.is_err());
        let violations = result.unwrap_err();
        assert!(violations.iter().any(|v| matches!(v, ContractViolation::WrongArity { function, expected: 0, actual: 1 } if function == "init_strategy")));
    }
}
