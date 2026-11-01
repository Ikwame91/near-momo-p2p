//! Test A part 2 (PLAN M0.4): a tiny contract that asks OutLayer to run a wasm and stores what
//! comes back in the callback. It is a probe, not the escrow.
//!
//! OutLayer facts this relies on (OutLayer contract source, github.com/out-layer/outlayer,
//! contract/src/execution.rs, and https://app.outlayer.ai/docs/near-integration):
//! - `request_execution` yields and resumes; its final value is what our `.then` callback sees.
//! - Run succeeded: the program's output as JSON (`response_format: "Json"`).
//! - Run failed: JSON `null`.
//! - Yield timed out / promise error: OutLayer returns no value at all (empty bytes).

use near_sdk::serde::Serialize;
use near_sdk::serde_json::{self, json, Value};
use near_sdk::{env, near, AccountId, Gas, NearToken, Promise, PromiseError};

/// Gas for the OutLayer call. OutLayer reserves at least 50 TGas of it for its own resume
/// callback (contract/src/lib.rs `MIN_RESPONSE_GAS`).
const OUTLAYER_GAS: Gas = Gas::from_tgas(200);
const CALLBACK_GAS: Gas = Gas::from_tgas(30);
/// Largest callback result we read; bigger results are treated as NoValue (never as output).
const MAX_RESULT_BYTES: usize = 4096;

/// What the callback received, sorted into the only cases that matter to the escrow.
#[derive(Debug, PartialEq, Serialize)]
#[serde(crate = "near_sdk::serde", tag = "kind", content = "value")]
pub enum Outcome {
    /// The program ran and printed this JSON. Only this case may ever lead to PAID.
    Output(Value),
    /// OutLayer says the run failed (`null`).
    RunFailed,
    /// No value (e.g. yield timeout), bytes that are not JSON, or a result over MAX_RESULT_BYTES.
    NoValue,
    /// The promise itself failed.
    PromiseFailed,
}

pub fn classify(result: Result<Vec<u8>, PromiseError>) -> Outcome {
    match result {
        Ok(bytes) if bytes.is_empty() => Outcome::NoValue,
        Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
            Ok(Value::Null) => Outcome::RunFailed,
            Ok(v) => Outcome::Output(v),
            Err(_) => Outcome::NoValue,
        },
        Err(PromiseError::TooLong(_)) => Outcome::NoValue,
        // Failed, or any error kind added to the SDK later: fail closed.
        Err(_) => Outcome::PromiseFailed,
    }
}

#[near(serializers = [json, borsh])]
#[derive(Clone, Debug)]
pub struct Record {
    pub id: u64,
    /// `Outcome` as JSON text, so the view shows it as-is.
    pub outcome: String,
    pub block_height: u64,
}

#[near(contract_state)]
pub struct Contract {
    outlayer: AccountId,
    next_id: u64,
    records: Vec<Record>,
}

impl Default for Contract {
    fn default() -> Self {
        Self { outlayer: "outlayer.testnet".parse().unwrap(), next_id: 0, records: Vec::new() }
    }
}

#[near]
impl Contract {
    /// Ask OutLayer to run `wasm_url` (checked against `wasm_hash`) with `input`.
    /// Attach the OutLayer fee (see `estimate_execution_cost` on outlayer.testnet); any
    /// unused part is refunded by OutLayer to the signer.
    #[payable]
    pub fn request(&mut self, wasm_url: String, wasm_hash: String, input: String) -> Promise {
        let id = self.next_id;
        self.next_id += 1;
        let args = json!({
            "source": {"WasmUrl": {"url": wasm_url, "hash": wasm_hash, "build_target": "wasm32-wasip2"}},
            "resource_limits": {"max_instructions": 1_000_000_000u64, "max_memory_mb": 128, "max_execution_seconds": 20},
            "input_data": input,
            "secrets_ref": null,
            "response_format": "Json",
            "payer_account_id": null,
            "params": null
        });
        Promise::new(self.outlayer.clone())
            .function_call(
                "request_execution".to_string(),
                args.to_string().into_bytes(),
                env::attached_deposit(),
                OUTLAYER_GAS,
            )
            .then(
                Promise::new(env::current_account_id()).function_call(
                    "on_result".to_string(),
                    json!({ "id": id }).to_string().into_bytes(),
                    NearToken::from_yoctonear(0),
                    CALLBACK_GAS,
                ),
            )
    }

    /// Callback. `#[private]` = only this contract itself may call it (predecessor must equal
    /// current account), which is what stops an outsider from posting a fake result (T13).
    #[private]
    pub fn on_result(&mut self, id: u64) -> String {
        let outcome = classify(env::promise_result_checked(0, MAX_RESULT_BYTES));
        let text = serde_json::to_string(&outcome).unwrap();
        self.records.push(Record { id, outcome: text.clone(), block_height: env::block_height() });
        text
    }

    pub fn get_records(&self) -> Vec<Record> {
        self.records.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(s: &str) -> Result<Vec<u8>, PromiseError> {
        Ok(s.as_bytes().to_vec())
    }

    #[test]
    fn output_json_is_output() {
        assert_eq!(classify(ok(r#"{"success":true}"#)), Outcome::Output(json!({"success": true})));
    }

    #[test]
    fn null_is_run_failed_not_output() {
        assert_eq!(classify(ok("null")), Outcome::RunFailed);
    }

    #[test]
    fn empty_is_no_value() {
        assert_eq!(classify(ok("")), Outcome::NoValue);
    }

    #[test]
    fn garbage_is_no_value() {
        assert_eq!(classify(ok("not json")), Outcome::NoValue);
    }

    #[test]
    fn promise_failure_is_promise_failed() {
        assert_eq!(classify(Err(PromiseError::Failed)), Outcome::PromiseFailed);
    }

    #[test]
    fn too_long_is_no_value_not_output() {
        assert_eq!(classify(Err(PromiseError::TooLong(5000))), Outcome::NoValue);
    }
}
