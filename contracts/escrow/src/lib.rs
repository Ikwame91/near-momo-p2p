//! OpenRamp escrow contract (PLAN M1). See docs/SPEC.md.
//!
//! M1.1 scaffold: deploy-time configuration only. Everything here is fixed at `new` and has no
//! setter (SPEC invariant 9). There is no owner role (DECISIONS D-014): nobody, including the
//! deployer, can change the token, the OutLayer contract, the verifier, the parameters or the
//! `deploy_tag` (which makes payment references unique across deployments, invariant 12).
//! LPs, orders and verdicts arrive in M1.2+.

use near_sdk::json_types::U128;
use near_sdk::store::LookupMap;
use near_sdk::{env, near, require, AccountId, BorshStorageKey, PanicOnDefault};

mod lp;
pub use lp::{Lp, Pot, Quote, SecretRef};

#[cfg(test)]
mod lp_tests;

#[near]
#[derive(BorshStorageKey)]
enum StorageKey {
    Lps,
}

/// Which OutLayer program checks payments. Fixed at deploy (invariant 9, T21, T26).
#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, PartialEq)]
pub enum VerifierSource {
    /// A wasm file at a URL, pinned by its sha256 (64 lowercase hex chars).
    WasmUrl { url: String, hash: String },
    /// An OutLayer project at a fixed version.
    Project { project_id: String, version_key: String },
}

/// What the contract attaches to every OutLayer call. Set here, never by the caller, so a caller
/// cannot make a verification fail by under-funding it (invariant 11, T25).
/// Limits from OutLayer's `request_execution` docs (https://app.outlayer.ai/docs/near-integration):
/// max 500e9 instructions, 4096 MB, 180 s. OutLayer reserves at least 50 TGas of the call's gas
/// for its own resume callback (outlayer contract/src/lib.rs `MIN_RESPONSE_GAS`).
#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, PartialEq)]
pub struct OutlayerCall {
    /// yoctoNEAR attached to `request_execution`; OutLayer refunds what the run does not use.
    pub fee: U128,
    /// Gas for the `request_execution` call (TGas).
    pub gas_tgas: u64,
    /// Gas for our own callback that receives the verdict (TGas).
    pub callback_gas_tgas: u64,
    pub max_instructions: u64,
    pub max_memory_mb: u32,
    pub max_execution_seconds: u32,
}

pub const OUTLAYER_MAX_INSTRUCTIONS: u64 = 500_000_000_000;
pub const OUTLAYER_MAX_MEMORY_MB: u32 = 4096;
pub const OUTLAYER_MAX_SECONDS: u32 = 180;
/// OutLayer keeps >= 50 TGas for its resume; we require headroom above that for the run's
/// bookkeeping (M0.4 used 200 TGas successfully).
pub const MIN_OUTLAYER_GAS_TGAS: u64 = 100;
pub const MIN_CALLBACK_GAS_TGAS: u64 = 10;
/// SPEC §7 range for the OutLayer call itself.
pub const MAX_OUTLAYER_GAS_TGAS: u64 = 250;
/// Ceilings: settings are permanent, so absurd values (e.g. a 100-year outage window that makes
/// slashing meaningless) must be impossible to deploy (SPEC §7).
pub const NS_PER_HOUR: u64 = 3_600_000_000_000;
pub const MAX_T_PAY_NS: u64 = 24 * NS_PER_HOUR;
pub const MAX_T_GRACE_NS: u64 = 24 * NS_PER_HOUR;
pub const MAX_T_RETRY_NS: u64 = 7 * 24 * NS_PER_HOUR;
pub const MAX_T_OUTAGE_NS: u64 = 7 * 24 * NS_PER_HOUR;
pub const MAX_RELEASE_DELAY_NS: u64 = 24 * NS_PER_HOUR;
pub const MAX_OUTLAYER_FEE_YOCTO: u128 = 1_000_000_000_000_000_000_000_000; // 1 NEAR
/// A slash needs >= 2 failures with the first and last a full window apart (SPEC §4.5); with 1 the
/// window could never be met.
pub const MIN_FAILED_ATTEMPTS: u8 = 2;
pub const MAX_FAILED_ATTEMPTS: u8 = 10;
/// A transaction can carry at most 300 TGas; leave room for `claim` itself.
pub const MAX_TOTAL_CALL_GAS_TGAS: u64 = 280;

/// Timing and cap parameters (SPEC §7). Durations are in nanoseconds (NEAR block timestamps).
#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, PartialEq)]
pub struct Params {
    pub t_pay_ns: u64,
    pub t_grace_ns: u64,
    /// Slash/refund window when a failure is the LP's fault (D-012).
    pub t_retry_ns: u64,
    /// Slash/refund window when every failure is an outage; hard limit (D-012).
    pub t_outage_max_ns: u64,
    pub min_failed_attempts: u8,
    pub new_buyer_cap: U128,
    pub release_delay_ns: u64,
    pub outlayer_call: OutlayerCall,
}

#[near(serializers = [json])]
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigView {
    pub token: AccountId,
    pub outlayer: AccountId,
    pub verifier: VerifierSource,
    pub params: Params,
    pub deploy_tag: String,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Escrow {
    token: AccountId,
    outlayer: AccountId,
    verifier: VerifierSource,
    params: Params,
    deploy_tag: String,
    lps: LookupMap<AccountId, Lp>,
}

/// First 8 hex chars of sha256(contract_id ":" init_block_height), height in decimal
/// (SPEC §4.1). The ":" keeps different (id, height) pairs from producing the same input text.
pub fn compute_deploy_tag(contract_id: &AccountId, block_height: u64) -> String {
    let digest = env::sha256(format!("{contract_id}:{block_height}").as_bytes());
    digest[..4].iter().map(|b| format!("{b:02x}")).collect()
}

fn validate(me: &AccountId, token: &AccountId, outlayer: &AccountId, verifier: &VerifierSource, p: &Params) {
    require!(token != outlayer, "token and outlayer must be different accounts");
    require!(token != me && outlayer != me, "token and outlayer must not be this contract");
    match verifier {
        VerifierSource::WasmUrl { url, hash } => {
            require!(url.starts_with("https://"), "verifier url must be https");
            require!(
                hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                "verifier hash must be 64 lowercase hex chars"
            );
        }
        VerifierSource::Project { project_id, version_key } => {
            require!(!project_id.is_empty() && !version_key.is_empty(), "verifier project and version required");
        }
    }
    require!(p.t_pay_ns > 0, "t_pay must be > 0");
    require!(p.t_retry_ns > 0, "t_retry must be > 0");
    require!(p.t_outage_max_ns >= p.t_retry_ns, "t_outage_max must be >= t_retry");
    require!(p.t_pay_ns <= MAX_T_PAY_NS, "t_pay too long");
    require!(p.t_grace_ns <= MAX_T_GRACE_NS, "t_grace too long");
    require!(p.t_retry_ns <= MAX_T_RETRY_NS, "t_retry too long");
    require!(p.t_outage_max_ns <= MAX_T_OUTAGE_NS, "t_outage_max too long");
    require!(p.release_delay_ns <= MAX_RELEASE_DELAY_NS, "release_delay too long");
    require!(
        (MIN_FAILED_ATTEMPTS..=MAX_FAILED_ATTEMPTS).contains(&p.min_failed_attempts),
        "min_failed_attempts must be 2-10"
    );
    require!(p.new_buyer_cap.0 > 0, "new_buyer_cap must be > 0");
    let c = &p.outlayer_call;
    require!(c.fee.0 > 0, "outlayer fee must be > 0");
    require!(c.fee.0 <= MAX_OUTLAYER_FEE_YOCTO, "outlayer fee too high");
    require!(c.gas_tgas >= MIN_OUTLAYER_GAS_TGAS, "outlayer gas too low");
    require!(c.gas_tgas <= MAX_OUTLAYER_GAS_TGAS, "outlayer gas too high");
    require!(c.callback_gas_tgas >= MIN_CALLBACK_GAS_TGAS, "callback gas too low");
    require!(c.gas_tgas + c.callback_gas_tgas <= MAX_TOTAL_CALL_GAS_TGAS, "outlayer + callback gas too high");
    require!(
        c.max_instructions >= 1 && c.max_instructions <= OUTLAYER_MAX_INSTRUCTIONS,
        "max_instructions out of range"
    );
    require!(c.max_memory_mb >= 1 && c.max_memory_mb <= OUTLAYER_MAX_MEMORY_MB, "max_memory_mb out of range");
    require!(
        c.max_execution_seconds >= 1 && c.max_execution_seconds <= OUTLAYER_MAX_SECONDS,
        "max_execution_seconds out of range"
    );
}

#[near]
impl Escrow {
    /// Only this contract's own account can initialise it, so nobody can front-run a deploy and
    /// initialise our address with their own verifier. Deploy and `new` must be one batched
    /// transaction signed by the contract account (PLAN M2.7). Checked in the body (not only with
    /// `#[private]`, which is enforced in the wasm entry point and is invisible to unit tests).
    #[init]
    pub fn new(token: AccountId, outlayer: AccountId, verifier: VerifierSource, params: Params) -> Self {
        let me = env::current_account_id();
        require!(env::predecessor_account_id() == me, "only the contract account can initialise it");
        validate(&me, &token, &outlayer, &verifier, &params);
        let deploy_tag = compute_deploy_tag(&me, env::block_height());
        Self { token, outlayer, verifier, params, deploy_tag, lps: LookupMap::new(StorageKey::Lps) }
    }

    pub fn get_config(&self) -> ConfigView {
        ConfigView {
            token: self.token.clone(),
            outlayer: self.outlayer.clone(),
            verifier: self.verifier.clone(),
            params: self.params.clone(),
            deploy_tag: self.deploy_tag.clone(),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::testing_env;

    const MIN: u64 = 60_000_000_000;
    pub(crate) const ME: &str = "escrow.testnet";

    pub(crate) fn acc(s: &str) -> AccountId {
        s.parse().unwrap()
    }

    pub(crate) fn params() -> Params {
        // Demo values from SPEC §7; OutLayer values as used in Test A (M0.4/M0.5).
        Params {
            t_pay_ns: 10 * MIN,
            t_grace_ns: 2 * MIN,
            t_retry_ns: 3 * MIN,
            t_outage_max_ns: 6 * MIN,
            min_failed_attempts: 2,
            new_buyer_cap: U128(50_000_000),
            release_delay_ns: 0,
            outlayer_call: OutlayerCall {
                fee: U128(10_000_000_000_000_000_000_000),
                gas_tgas: 200,
                callback_gas_tgas: 30,
                max_instructions: 1_000_000_000,
                max_memory_mb: 128,
                max_execution_seconds: 20,
            },
        }
    }

    pub(crate) fn verifier() -> VerifierSource {
        VerifierSource::WasmUrl {
            url: "https://raw.githubusercontent.com/Ikwame91/near-momo-p2p/abc/wasm/verifier.wasm".into(),
            hash: "021f8568f2cc9001e7060fedb02786fb02212b5e8ad8ffedd3f35c98b3d49c61".into(),
        }
    }

    /// Context where the contract calls itself (as in a deploy + init batch).
    pub(crate) fn ctx(height: u64) {
        testing_env!(VMContextBuilder::new()
            .current_account_id(acc(ME))
            .predecessor_account_id(acc(ME))
            .block_height(height)
            .build());
    }

    pub(crate) fn init_with(verifier: VerifierSource, p: Params) -> Escrow {
        Escrow::new(acc("usdc.testnet"), acc("outlayer.testnet"), verifier, p)
    }

    fn init_params(p: Params) -> Escrow {
        ctx(1);
        init_with(verifier(), p)
    }

    #[test]
    fn init_stores_fixed_config() {
        ctx(100);
        let c = init_with(verifier(), params()).get_config();
        assert_eq!(c.token, acc("usdc.testnet"));
        assert_eq!(c.outlayer, acc("outlayer.testnet"));
        assert_eq!(c.verifier, verifier());
        assert_eq!(c.params, params());
        assert_eq!(c.deploy_tag, compute_deploy_tag(&acc(ME), 100));
    }

    #[test]
    #[should_panic(expected = "only the contract account can initialise it")]
    fn init_by_another_account_is_rejected() {
        testing_env!(VMContextBuilder::new()
            .current_account_id(acc(ME))
            .predecessor_account_id(acc("attacker.testnet"))
            .build());
        init_with(verifier(), params());
    }

    #[test]
    fn deploy_tag_is_8_hex_and_differs_per_deployment() {
        let a = compute_deploy_tag(&acc(ME), 100);
        assert_eq!(a.len(), 8);
        assert!(a.bytes().all(|b| b.is_ascii_hexdigit()));
        // Same account redeployed later (order ids restart) → different tag (T27).
        assert_ne!(a, compute_deploy_tag(&acc(ME), 101));
        // Another instance → different tag.
        assert_ne!(a, compute_deploy_tag(&acc("escrow2.testnet"), 100));
        // Deterministic.
        assert_eq!(a, compute_deploy_tag(&acc(ME), 100));
    }

    #[test]
    fn deploy_tag_matches_spec_formula() {
        // SPEC §4.1: first 8 hex of sha256("escrow.testnet:100").
        let digest = env::sha256(b"escrow.testnet:100");
        let expected: String = digest[..4].iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(compute_deploy_tag(&acc(ME), 100), expected);
    }

    // ---- account checks ----

    #[test]
    #[should_panic(expected = "token and outlayer must be different accounts")]
    fn rejects_token_equal_outlayer() {
        ctx(1);
        Escrow::new(acc("same.testnet"), acc("same.testnet"), verifier(), params());
    }

    #[test]
    #[should_panic(expected = "token and outlayer must not be this contract")]
    fn rejects_token_equal_self() {
        ctx(1);
        Escrow::new(acc(ME), acc("outlayer.testnet"), verifier(), params());
    }

    #[test]
    #[should_panic(expected = "token and outlayer must not be this contract")]
    fn rejects_outlayer_equal_self() {
        ctx(1);
        Escrow::new(acc("usdc.testnet"), acc(ME), verifier(), params());
    }

    // ---- verifier checks ----

    #[test]
    #[should_panic(expected = "verifier hash must be 64 lowercase hex chars")]
    fn rejects_short_hash() {
        ctx(1);
        init_with(VerifierSource::WasmUrl { url: "https://x.example/v.wasm".into(), hash: "abc".into() }, params());
    }

    #[test]
    #[should_panic(expected = "verifier hash must be 64 lowercase hex chars")]
    fn rejects_uppercase_hash_of_right_length() {
        ctx(1);
        let hash = "021F8568F2CC9001E7060FEDB02786FB02212B5E8AD8FFEDD3F35C98B3D49C61".to_string();
        assert_eq!(hash.len(), 64);
        init_with(VerifierSource::WasmUrl { url: "https://x.example/v.wasm".into(), hash }, params());
    }

    #[test]
    #[should_panic(expected = "verifier hash must be 64 lowercase hex chars")]
    fn rejects_non_hex_hash_of_right_length() {
        ctx(1);
        init_with(VerifierSource::WasmUrl { url: "https://x.example/v.wasm".into(), hash: "g".repeat(64) }, params());
    }

    #[test]
    #[should_panic(expected = "verifier url must be https")]
    fn rejects_http_verifier_url() {
        ctx(1);
        init_with(VerifierSource::WasmUrl { url: "http://x.example/v.wasm".into(), hash: "0".repeat(64) }, params());
    }

    #[test]
    #[should_panic(expected = "verifier project and version required")]
    fn rejects_project_with_empty_id() {
        ctx(1);
        init_with(VerifierSource::Project { project_id: "".into(), version_key: "v1".into() }, params());
    }

    #[test]
    #[should_panic(expected = "verifier project and version required")]
    fn rejects_project_with_empty_version() {
        ctx(1);
        init_with(VerifierSource::Project { project_id: "a.testnet/v".into(), version_key: "".into() }, params());
    }

    #[test]
    fn accepts_project_verifier() {
        ctx(1);
        let v = VerifierSource::Project { project_id: "a.testnet/verifier".into(), version_key: "abc".into() };
        assert_eq!(init_with(v.clone(), params()).get_config().verifier, v);
    }

    // ---- timing checks ----

    #[test]
    #[should_panic(expected = "t_pay must be > 0")]
    fn rejects_zero_t_pay() {
        let mut p = params();
        p.t_pay_ns = 0;
        init_params(p);
    }

    #[test]
    #[should_panic(expected = "t_retry must be > 0")]
    fn rejects_zero_t_retry() {
        let mut p = params();
        p.t_retry_ns = 0;
        p.t_outage_max_ns = 0;
        init_params(p);
    }

    #[test]
    #[should_panic(expected = "t_outage_max must be >= t_retry")]
    fn rejects_outage_window_shorter_than_retry() {
        let mut p = params();
        p.t_outage_max_ns = p.t_retry_ns - 1;
        init_params(p);
    }

    #[test]
    fn accepts_outage_window_equal_to_retry() {
        let mut p = params();
        p.t_outage_max_ns = p.t_retry_ns;
        init_params(p);
    }

    #[test]
    #[should_panic(expected = "min_failed_attempts must be 2-10")]
    fn rejects_zero_failed_attempts() {
        let mut p = params();
        p.min_failed_attempts = 0;
        init_params(p);
    }

    // ---- OutLayer call checks (invariant 11) ----

    #[test]
    #[should_panic(expected = "outlayer fee must be > 0")]
    fn rejects_zero_outlayer_fee() {
        let mut p = params();
        p.outlayer_call.fee = U128(0);
        init_params(p);
    }

    #[test]
    #[should_panic(expected = "outlayer gas too low")]
    fn rejects_low_outlayer_gas() {
        let mut p = params();
        p.outlayer_call.gas_tgas = MIN_OUTLAYER_GAS_TGAS - 1;
        init_params(p);
    }

    #[test]
    #[should_panic(expected = "callback gas too low")]
    fn rejects_low_callback_gas() {
        let mut p = params();
        p.outlayer_call.callback_gas_tgas = MIN_CALLBACK_GAS_TGAS - 1;
        init_params(p);
    }

    #[test]
    #[should_panic(expected = "outlayer + callback gas too high")]
    fn rejects_total_gas_over_limit() {
        let mut p = params();
        // Each value is within its own range; only the sum (281) is over the limit.
        p.outlayer_call.gas_tgas = MAX_OUTLAYER_GAS_TGAS;
        p.outlayer_call.callback_gas_tgas = 31;
        init_params(p);
    }

    #[test]
    #[should_panic(expected = "max_instructions out of range")]
    fn rejects_too_many_instructions() {
        let mut p = params();
        p.outlayer_call.max_instructions = OUTLAYER_MAX_INSTRUCTIONS + 1;
        init_params(p);
    }

    #[test]
    #[should_panic(expected = "max_instructions out of range")]
    fn rejects_zero_instructions() {
        let mut p = params();
        p.outlayer_call.max_instructions = 0;
        init_params(p);
    }

    #[test]
    #[should_panic(expected = "max_memory_mb out of range")]
    fn rejects_too_much_memory() {
        let mut p = params();
        p.outlayer_call.max_memory_mb = OUTLAYER_MAX_MEMORY_MB + 1;
        init_params(p);
    }

    #[test]
    #[should_panic(expected = "max_execution_seconds out of range")]
    fn rejects_too_many_seconds() {
        let mut p = params();
        p.outlayer_call.max_execution_seconds = OUTLAYER_MAX_SECONDS + 1;
        init_params(p);
    }

    #[test]
    #[should_panic(expected = "min_failed_attempts must be 2-10")]
    fn rejects_one_failed_attempt() {
        let mut p = params();
        p.min_failed_attempts = 1;
        init_params(p);
    }

    #[test]
    #[should_panic(expected = "min_failed_attempts must be 2-10")]
    fn rejects_eleven_failed_attempts() {
        let mut p = params();
        p.min_failed_attempts = 11;
        init_params(p);
    }

    #[test]
    #[should_panic(expected = "outlayer gas too high")]
    fn rejects_outlayer_gas_above_250() {
        let mut p = params();
        p.outlayer_call.gas_tgas = 251;
        p.outlayer_call.callback_gas_tgas = 10;
        init_params(p);
    }

    #[test]
    #[should_panic(expected = "t_outage_max too long")]
    fn rejects_outage_window_over_7_days() {
        let mut p = params();
        p.t_outage_max_ns = MAX_T_OUTAGE_NS + 1;
        init_params(p);
    }

    #[test]
    #[should_panic(expected = "t_retry too long")]
    fn rejects_retry_window_over_7_days() {
        let mut p = params();
        p.t_retry_ns = MAX_T_RETRY_NS + 1;
        p.t_outage_max_ns = MAX_T_RETRY_NS + 1;
        init_params(p);
    }

    #[test]
    #[should_panic(expected = "t_pay too long")]
    fn rejects_t_pay_over_24h() {
        let mut p = params();
        p.t_pay_ns = MAX_T_PAY_NS + 1;
        init_params(p);
    }

    #[test]
    #[should_panic(expected = "t_grace too long")]
    fn rejects_t_grace_over_24h() {
        let mut p = params();
        p.t_grace_ns = MAX_T_GRACE_NS + 1;
        init_params(p);
    }

    #[test]
    #[should_panic(expected = "release_delay too long")]
    fn rejects_release_delay_over_24h() {
        let mut p = params();
        p.release_delay_ns = MAX_RELEASE_DELAY_NS + 1;
        init_params(p);
    }

    #[test]
    #[should_panic(expected = "outlayer fee too high")]
    fn rejects_outlayer_fee_over_1_near() {
        let mut p = params();
        p.outlayer_call.fee = U128(MAX_OUTLAYER_FEE_YOCTO + 1);
        init_params(p);
    }

    #[test]
    #[should_panic(expected = "new_buyer_cap must be > 0")]
    fn rejects_zero_new_buyer_cap() {
        let mut p = params();
        p.new_buyer_cap = U128(0);
        init_params(p);
    }

    #[test]
    fn accepts_production_values() {
        // SPEC §7 prod column: T_retry 24 h, T_outage_max 72 h, 3 attempts, release delay 1 h.
        let mut p = params();
        p.t_pay_ns = NS_PER_HOUR / 2;
        p.t_grace_ns = NS_PER_HOUR / 4;
        p.t_retry_ns = 24 * NS_PER_HOUR;
        p.t_outage_max_ns = 72 * NS_PER_HOUR;
        p.min_failed_attempts = 3;
        p.release_delay_ns = NS_PER_HOUR;
        init_params(p);
    }

    #[test]
    fn accepts_limits_at_outlayer_maximums() {
        let mut p = params();
        p.outlayer_call.max_instructions = OUTLAYER_MAX_INSTRUCTIONS;
        p.outlayer_call.max_memory_mb = OUTLAYER_MAX_MEMORY_MB;
        p.outlayer_call.max_execution_seconds = OUTLAYER_MAX_SECONDS;
        p.outlayer_call.gas_tgas = 250;
        p.outlayer_call.callback_gas_tgas = 30;
        init_params(p);
    }
}
