//! OpenRamp escrow contract (PLAN M1). See docs/SPEC.md.
//!
//! M1.1 scaffold: deploy-time configuration only. Everything here is fixed at `new` and has no
//! setter (SPEC invariant 9): the accepted token, the OutLayer contract, the verifier source and
//! the `deploy_tag` that makes payment references unique across deployments (invariant 12).
//! LPs, orders and verdicts arrive in M1.2+.

use near_sdk::json_types::U128;
use near_sdk::{env, near, require, AccountId, PanicOnDefault};

/// Which OutLayer program checks payments. Fixed at deploy (invariant 9, T21, T26).
#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, PartialEq)]
pub enum VerifierSource {
    /// A wasm file at a URL, pinned by its sha256 (64 lowercase hex chars).
    WasmUrl { url: String, hash: String },
    /// An OutLayer project at a fixed version.
    Project { project_id: String, version_key: String },
}

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
    /// yoctoNEAR the contract attaches to each OutLayer call (invariant 11).
    pub outlayer_fee: U128,
}

#[near(serializers = [json])]
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigView {
    pub owner: AccountId,
    pub token: AccountId,
    pub outlayer: AccountId,
    pub verifier: VerifierSource,
    pub params: Params,
    pub deploy_tag: String,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Escrow {
    owner: AccountId,
    token: AccountId,
    outlayer: AccountId,
    verifier: VerifierSource,
    params: Params,
    deploy_tag: String,
}

/// First 8 hex chars of sha256(contract_id ‖ ":" ‖ init_block_height) (SPEC §4.1).
pub fn compute_deploy_tag(contract_id: &AccountId, block_height: u64) -> String {
    let digest = env::sha256(format!("{contract_id}:{block_height}").as_bytes());
    digest[..4].iter().map(|b| format!("{b:02x}")).collect()
}

fn validate(verifier: &VerifierSource, p: &Params) {
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
    require!(p.min_failed_attempts >= 1, "min_failed_attempts must be >= 1");
    require!(p.outlayer_fee.0 > 0, "outlayer_fee must be > 0");
}

#[near]
impl Escrow {
    #[init]
    pub fn new(
        owner: AccountId,
        token: AccountId,
        outlayer: AccountId,
        verifier: VerifierSource,
        params: Params,
    ) -> Self {
        validate(&verifier, &params);
        let deploy_tag = compute_deploy_tag(&env::current_account_id(), env::block_height());
        Self { owner, token, outlayer, verifier, params, deploy_tag }
    }

    pub fn get_config(&self) -> ConfigView {
        ConfigView {
            owner: self.owner.clone(),
            token: self.token.clone(),
            outlayer: self.outlayer.clone(),
            verifier: self.verifier.clone(),
            params: self.params.clone(),
            deploy_tag: self.deploy_tag.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::testing_env;

    const MIN: u64 = 60_000_000_000;

    fn acc(s: &str) -> AccountId {
        s.parse().unwrap()
    }

    fn params() -> Params {
        // Demo values from SPEC §7.
        Params {
            t_pay_ns: 10 * MIN,
            t_grace_ns: 2 * MIN,
            t_retry_ns: 3 * MIN,
            t_outage_max_ns: 6 * MIN,
            min_failed_attempts: 2,
            new_buyer_cap: U128(50_000_000),
            release_delay_ns: 0,
            outlayer_fee: U128(10_000_000_000_000_000_000_000),
        }
    }

    fn verifier() -> VerifierSource {
        VerifierSource::WasmUrl {
            url: "https://raw.githubusercontent.com/Ikwame91/near-momo-p2p/abc/wasm/verifier.wasm".into(),
            hash: "021f8568f2cc9001e7060fedb02786fb02212b5e8ad8ffedd3f35c98b3d49c61".into(),
        }
    }

    fn ctx(contract: &str, height: u64) {
        testing_env!(VMContextBuilder::new()
            .current_account_id(acc(contract))
            .block_height(height)
            .build());
    }

    fn deploy() -> Escrow {
        Escrow::new(acc("owner.testnet"), acc("usdc.testnet"), acc("outlayer.testnet"), verifier(), params())
    }

    #[test]
    fn init_stores_fixed_config() {
        ctx("escrow.testnet", 100);
        let c = deploy().get_config();
        assert_eq!(c.owner, acc("owner.testnet"));
        assert_eq!(c.token, acc("usdc.testnet"));
        assert_eq!(c.outlayer, acc("outlayer.testnet"));
        assert_eq!(c.verifier, verifier());
        assert_eq!(c.params, params());
        assert_eq!(c.deploy_tag, compute_deploy_tag(&acc("escrow.testnet"), 100));
    }

    #[test]
    fn deploy_tag_is_8_hex_and_differs_per_deployment() {
        let a = compute_deploy_tag(&acc("escrow.testnet"), 100);
        assert_eq!(a.len(), 8);
        assert!(a.bytes().all(|b| b.is_ascii_hexdigit()));
        // Same account redeployed later (order ids restart) → different tag (T27).
        assert_ne!(a, compute_deploy_tag(&acc("escrow.testnet"), 101));
        // Another instance → different tag.
        assert_ne!(a, compute_deploy_tag(&acc("escrow2.testnet"), 100));
        // Deterministic.
        assert_eq!(a, compute_deploy_tag(&acc("escrow.testnet"), 100));
    }

    #[test]
    #[should_panic(expected = "verifier hash must be 64 lowercase hex chars")]
    fn rejects_bad_verifier_hash() {
        ctx("escrow.testnet", 1);
        let v = VerifierSource::WasmUrl { url: "https://x.example/v.wasm".into(), hash: "ABC".into() };
        Escrow::new(acc("o.testnet"), acc("t.testnet"), acc("outlayer.testnet"), v, params());
    }

    #[test]
    #[should_panic(expected = "verifier url must be https")]
    fn rejects_http_verifier_url() {
        ctx("escrow.testnet", 1);
        let v = VerifierSource::WasmUrl { url: "http://x.example/v.wasm".into(), hash: "0".repeat(64) };
        Escrow::new(acc("o.testnet"), acc("t.testnet"), acc("outlayer.testnet"), v, params());
    }

    #[test]
    #[should_panic(expected = "t_outage_max must be >= t_retry")]
    fn rejects_outage_window_shorter_than_retry() {
        ctx("escrow.testnet", 1);
        let mut p = params();
        p.t_outage_max_ns = p.t_retry_ns - 1;
        Escrow::new(acc("o.testnet"), acc("t.testnet"), acc("outlayer.testnet"), verifier(), p);
    }

    #[test]
    #[should_panic(expected = "min_failed_attempts must be >= 1")]
    fn rejects_zero_failed_attempts() {
        ctx("escrow.testnet", 1);
        let mut p = params();
        p.min_failed_attempts = 0;
        Escrow::new(acc("o.testnet"), acc("t.testnet"), acc("outlayer.testnet"), verifier(), p);
    }
}
