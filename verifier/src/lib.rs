//! OpenRamp verifier: pure decision logic (no I/O), unit-tested natively. `main.rs` does the HTTP.
//!
//! Modes (SPEC §3.1, §4.2, §4.4):
//! - `fingerprint`: return `key_fp = sha256(key)` (hex) of the LP's provider key. No HTTP.
//! - `checkout`: create the Paystack checkout for the order with the LP's key
//!   (`POST /transaction/initialize`, GHS, `channels: ["mobile_money"]`, synthetic email), so payment
//!   and verification always use one Paystack account (T24).
//! - `verify`: `GET /transaction/verify/:reference` → `{verdict, reference, key_fp, reason}`.
//!
//! Paystack API: https://paystack.com/docs/api/transaction/ (initialize, verify). Observed in test
//! mode (M0.5): unknown reference → HTTP 400 with `code: "transaction_not_found"`.
//! Verdicts never treat "could not verify" as NOT_PAID (fail closed).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const VERIFY_URL: &str = "https://api.paystack.co/transaction/verify/";
pub const INITIALIZE_URL: &str = "https://api.paystack.co/transaction/initialize";
/// Synthetic email: Paystack requires one; it is not the buyer's (no PII, inv. 7).
pub const CHECKOUT_EMAIL: &str = "buyer@openramp.example";
/// The escrow stores checkout URLs up to this length (contract MAX_CHECKOUT_URL).
pub const MAX_CHECKOUT_URL: usize = 256;

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum Input {
    Fingerprint,
    Checkout {
        reference: String,
        amount_pesewas: u64,
        currency: String,
    },
    Verify {
        reference: String,
        expected_amount_pesewas: u64,
        currency: String,
        /// Unix seconds: the escrow passes `pay_deadline + t_grace` (D-022: payments inside the
        /// grace window count as on time).
        pay_deadline: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Verdict {
    Paid,
    NotPaid,
    Pending,
    Unverifiable,
}

/// `verify` output. Matches the escrow's `VerifierOutput`. Never contains customer data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifyOutput {
    pub verdict: Verdict,
    pub reference: String,
    pub key_fp: String,
    pub reason: String,
}

/// `fingerprint` and `checkout` output. `error` set means the mode failed (no key, HTTP error).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct InfoOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_fp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub fn key_fp(key: &str) -> String {
    Sha256::digest(key.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// Exactly `opr-<8 lowercase hex>-<digits>` (the escrow's format), so no other Paystack endpoint
/// can be reached through the URL path and no foreign reference is checked.
pub fn reference_is_valid(r: &str) -> bool {
    let Some(rest) = r.strip_prefix("opr-") else { return false };
    let Some((tag, id)) = rest.split_once('-') else { return false };
    tag.len() == 8
        && tag.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && !id.is_empty()
        && id.len() <= 20
        && id.bytes().all(|b| b.is_ascii_digit())
}

/// Paystack statuses for a charge still in flight (the buyer may still approve on the phone).
const PENDING_STATUSES: [&str; 9] =
    ["pending", "ongoing", "processing", "queued", "send_otp", "send_pin", "send_phone", "send_birthday", "pay_offline"];
/// Paystack statuses that clearly mean "not paid".
const NOT_PAID_STATUSES: [&str; 3] = ["failed", "abandoned", "reversed"];

/// Decide a `verify` from Paystack's HTTP status and body.
pub fn decide(reference: &str, expected: u64, currency: &str, deadline: u64, fp: &str, status: u16, body: &[u8]) -> VerifyOutput {
    let out = |verdict, reason: &str| VerifyOutput {
        verdict,
        reference: reference.to_string(),
        key_fp: fp.to_string(),
        reason: reason.to_string(),
    };
    match status {
        200 => {}
        // Only 401 is the LP's fault (key revoked/invalid). 403 is usually a WAF / outage (D-017).
        401 => return out(Verdict::Unverifiable, "auth_error"),
        400 | 404 => {
            let not_found = serde_json::from_slice::<Value>(body)
                .ok()
                .and_then(|v| v.get("code").and_then(|c| c.as_str()).map(|c| c == "transaction_not_found"))
                .unwrap_or(false);
            return if not_found { out(Verdict::NotPaid, "not_found") } else { out(Verdict::Unverifiable, "unclear_4xx") };
        }
        s if s >= 500 => return out(Verdict::Unverifiable, "server_error"),
        _ => return out(Verdict::Unverifiable, "unexpected_status"),
    }
    let v: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => return out(Verdict::Unverifiable, "malformed"),
    };
    if v.get("status").and_then(|s| s.as_bool()) != Some(true) {
        return out(Verdict::Unverifiable, "malformed");
    }
    let Some(d) = v.get("data").filter(|d| d.is_object()) else { return out(Verdict::Unverifiable, "malformed") };
    let Some(tx_status) = d.get("status").and_then(|s| s.as_str()) else { return out(Verdict::Unverifiable, "malformed") };
    if d.get("reference").and_then(|s| s.as_str()) != Some(reference) {
        return out(Verdict::NotPaid, "reference_mismatch");
    }
    // Amount, currency and channel are checked before PAID **and** before PENDING (PR #12 review),
    // so a tiny or card charge on our reference can neither pay nor keep the order alive.
    if d.get("currency").and_then(|s| s.as_str()) != Some(currency) {
        return out(Verdict::NotPaid, "currency_mismatch");
    }
    let Some(amount) = d.get("amount").and_then(|a| a.as_u64()) else { return out(Verdict::Unverifiable, "malformed") };
    if amount < expected {
        return out(Verdict::NotPaid, "amount_short");
    }
    // `channel` is absent until a payment method is chosen; when present it must be mobile money.
    if let Some(ch) = d.get("channel").and_then(|s| s.as_str()) {
        if ch != "mobile_money" {
            return out(Verdict::NotPaid, "wrong_channel");
        }
    }
    if NOT_PAID_STATUSES.contains(&tx_status) {
        return out(Verdict::NotPaid, "not_success");
    }
    if PENDING_STATUSES.contains(&tx_status) {
        return out(Verdict::Pending, "pending");
    }
    if tx_status != "success" {
        return out(Verdict::Unverifiable, "unknown_status");
    }
    if d.get("channel").and_then(|s| s.as_str()) != Some("mobile_money") {
        return out(Verdict::NotPaid, "wrong_channel");
    }
    let Some(paid_at) = d.get("paid_at").and_then(|s| s.as_str()).and_then(parse_iso8601_utc) else {
        return out(Verdict::Unverifiable, "malformed");
    };
    if paid_at > deadline {
        return out(Verdict::NotPaid, "late");
    }
    out(Verdict::Paid, "ok")
}

/// Decide a `checkout` from Paystack's initialize response: the authorization URL, or an error.
pub fn checkout_result(reference: &str, fp: &str, status: u16, body: &[u8]) -> InfoOutput {
    let err = |e: &str| InfoOutput { key_fp: Some(fp.into()), reference: Some(reference.into()), error: Some(e.into()), ..Default::default() };
    if status == 401 {
        return err("auth_error");
    }
    if status != 200 {
        return err("http_error");
    }
    let url = serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|v| v.get("data")?.get("authorization_url")?.as_str().map(str::to_string));
    match url {
        Some(u) if u.starts_with("https://checkout.paystack.com/") && u.len() <= MAX_CHECKOUT_URL => InfoOutput {
            key_fp: Some(fp.into()),
            authorization_url: Some(u),
            reference: Some(reference.into()),
            error: None,
        },
        _ => err("malformed"),
    }
}

/// Parse `YYYY-MM-DDTHH:MM:SS[.fff]Z` to Unix seconds. None for anything else.
pub fn parse_iso8601_utc(s: &str) -> Option<u64> {
    let b = s.as_bytes();
    if b.len() < 20 || *b.last()? != b'Z' {
        return None;
    }
    if b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let num = |from: usize, to: usize| -> Option<u64> { s.get(from..to)?.parse::<u64>().ok() };
    let (y, mo, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (h, mi, se) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    let rest = &s[19..s.len() - 1];
    if !(rest.is_empty() || (rest.starts_with('.') && rest.len() > 1 && rest[1..].bytes().all(|c| c.is_ascii_digit()))) {
        return None;
    }
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || se > 59 || y < 1970 {
        return None;
    }
    let (y, m) = (y as i64, mo as i64);
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146097 + doe - 719468) as u64 * 86400 + h * 3600 + mi * 60 + se)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REF: &str = "opr-1a2b3c4d-7";
    const PAID_AT: &str = "2026-11-01T20:48:20.000Z";
    const PAID_AT_UNIX: u64 = 1_793_566_100;
    const FP: &str = "ff";

    fn body(status: &str, amount: u64, currency: &str, channel: Option<&str>, paid_at: &str) -> Vec<u8> {
        let mut d = serde_json::json!({"status": status, "reference": REF, "amount": amount, "currency": currency, "paid_at": paid_at});
        if let Some(c) = channel {
            d["channel"] = c.into();
        }
        serde_json::json!({"status": true, "data": d}).to_string().into_bytes()
    }

    fn verify(status: u16, b: &[u8]) -> VerifyOutput {
        decide(REF, 1000, "GHS", PAID_AT_UNIX + 600, FP, status, b)
    }

    fn ok() -> Vec<u8> {
        body("success", 1000, "GHS", Some("mobile_money"), PAID_AT)
    }

    #[test]
    fn input_modes_parse() {
        assert_eq!(serde_json::from_str::<Input>(r#"{"mode":"fingerprint"}"#).unwrap(), Input::Fingerprint);
        let v: Input = serde_json::from_str(r#"{"mode":"verify","reference":"opr-1a2b3c4d-7","expected_amount_pesewas":1000,"currency":"GHS","pay_deadline":5}"#).unwrap();
        assert!(matches!(v, Input::Verify { .. }));
        let c: Input = serde_json::from_str(r#"{"mode":"checkout","reference":"opr-1a2b3c4d-7","amount_pesewas":1000,"currency":"GHS"}"#).unwrap();
        assert!(matches!(c, Input::Checkout { .. }));
        assert!(serde_json::from_str::<Input>(r#"{"mode":"steal"}"#).is_err());
    }

    #[test]
    fn key_fp_is_sha256_hex() {
        assert_eq!(key_fp("abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(key_fp("x").len(), 64);
    }

    #[test]
    fn reference_format_is_exact() {
        assert!(reference_is_valid("opr-1a2b3c4d-0"));
        assert!(reference_is_valid("opr-1a2b3c4d-123456"));
        for bad in ["opr-test-a-paid-1", "opr-1A2B3C4D-1", "opr-1a2b3c4-1", "opr-1a2b3c4d-", "opr-1a2b3c4d-1x",
                    "opr-1a2b3c4d-1/../x", "x-1a2b3c4d-1", "opr-1a2b3c4d"] {
            assert!(!reference_is_valid(bad), "{bad}");
        }
    }

    #[test]
    fn paid() {
        assert_eq!(verify(200, &ok()), VerifyOutput { verdict: Verdict::Paid, reference: REF.into(), key_fp: FP.into(), reason: "ok".into() });
    }

    #[test]
    fn every_output_carries_key_fp_and_reference() {
        for (s, b) in [(200, ok()), (401, vec![]), (500, vec![]), (400, b"{}".to_vec())] {
            let o = verify(s, &b);
            assert_eq!((o.key_fp.as_str(), o.reference.as_str()), (FP, REF));
        }
    }

    #[test]
    fn pending_statuses_are_pending() {
        for st in PENDING_STATUSES {
            assert_eq!(verify(200, &body(st, 1000, "GHS", None, "")).verdict, Verdict::Pending, "{st}");
        }
    }

    #[test]
    fn failed_abandoned_reversed_are_not_paid() {
        for st in NOT_PAID_STATUSES {
            assert_eq!(verify(200, &body(st, 1000, "GHS", None, "")).verdict, Verdict::NotPaid, "{st}");
        }
    }

    #[test]
    fn unknown_status_is_unverifiable() {
        assert_eq!(verify(200, &body("weird", 1000, "GHS", None, "")).reason, "unknown_status");
    }

    #[test]
    fn short_or_wrong_currency_pending_is_not_paid_not_pending() {
        // PR #12 review: a 1-pesewa pending charge cannot keep the order alive.
        assert_eq!(verify(200, &body("ongoing", 1, "GHS", None, "")).reason, "amount_short");
        assert_eq!(verify(200, &body("ongoing", 1000, "NGN", None, "")).reason, "currency_mismatch");
    }

    #[test]
    fn card_payment_is_not_paid() {
        // No cards (chargebacks); also a pending card charge cannot keep the order alive.
        assert_eq!(verify(200, &body("success", 1000, "GHS", Some("card"), PAID_AT)).reason, "wrong_channel");
        assert_eq!(verify(200, &body("ongoing", 1000, "GHS", Some("card"), "")).reason, "wrong_channel");
        assert_eq!(verify(200, &body("success", 1000, "GHS", None, PAID_AT)).reason, "wrong_channel");
    }

    #[test]
    fn late_and_boundary() {
        assert_eq!(decide(REF, 1000, "GHS", PAID_AT_UNIX - 1, FP, 200, &ok()).reason, "late");
        assert_eq!(decide(REF, 1000, "GHS", PAID_AT_UNIX, FP, 200, &ok()).verdict, Verdict::Paid);
    }

    #[test]
    fn only_401_is_auth_error() {
        assert_eq!(verify(401, b"").reason, "auth_error");
        assert_eq!(verify(403, b"").verdict, Verdict::Unverifiable);
        assert_ne!(verify(403, b"").reason, "auth_error", "403 is an outage (D-017)");
    }

    #[test]
    fn not_found_vs_other_4xx() {
        assert_eq!(verify(400, br#"{"code":"transaction_not_found"}"#).verdict, Verdict::NotPaid);
        assert_eq!(verify(400, br#"{"code":"other"}"#).verdict, Verdict::Unverifiable);
    }

    #[test]
    fn server_errors_and_malformed_are_unverifiable() {
        for s in [500, 502, 503, 429] {
            assert_eq!(verify(s, b"").verdict, Verdict::Unverifiable);
        }
        assert_eq!(verify(200, b"nope").verdict, Verdict::Unverifiable);
        assert_eq!(verify(200, br#"{"status":true}"#).verdict, Verdict::Unverifiable);
        assert_eq!(verify(200, &body("success", 1000, "GHS", Some("mobile_money"), "bad")).verdict, Verdict::Unverifiable);
    }

    #[test]
    fn other_reference_is_not_paid() {
        let b = serde_json::json!({"status": true, "data": {"status": "success", "reference": "opr-00000000-1", "amount": 1000, "currency": "GHS", "channel": "mobile_money", "paid_at": PAID_AT}});
        assert_eq!(verify(200, b.to_string().as_bytes()).reason, "reference_mismatch");
    }

    #[test]
    fn verify_output_json_matches_the_escrow() {
        let j = serde_json::to_value(verify(200, &ok())).unwrap();
        assert_eq!(j, serde_json::json!({"verdict": "PAID", "reference": REF, "key_fp": FP, "reason": "ok"}));
        assert_eq!(serde_json::to_value(Verdict::NotPaid).unwrap(), "NOT_PAID");
        assert_eq!(serde_json::to_value(Verdict::Pending).unwrap(), "PENDING");
    }

    #[test]
    fn checkout_returns_paystack_url() {
        let b = br#"{"status":true,"data":{"authorization_url":"https://checkout.paystack.com/abc123","reference":"opr-1a2b3c4d-7"}}"#;
        let o = checkout_result(REF, FP, 200, b);
        assert_eq!(o.authorization_url.as_deref(), Some("https://checkout.paystack.com/abc123"));
        assert_eq!(o.error, None);
    }

    #[test]
    fn checkout_rejects_foreign_or_long_urls_and_errors() {
        let foreign = br#"{"status":true,"data":{"authorization_url":"https://evil.example/pay"}}"#;
        assert_eq!(checkout_result(REF, FP, 200, foreign).error.as_deref(), Some("malformed"));
        let long = format!(r#"{{"status":true,"data":{{"authorization_url":"https://checkout.paystack.com/{}"}}}}"#, "a".repeat(300));
        assert_eq!(checkout_result(REF, FP, 200, long.as_bytes()).error.as_deref(), Some("malformed"));
        assert_eq!(checkout_result(REF, FP, 401, b"").error.as_deref(), Some("auth_error"));
        assert_eq!(checkout_result(REF, FP, 500, b"").error.as_deref(), Some("http_error"));
    }

    #[test]
    fn iso_parse() {
        assert_eq!(parse_iso8601_utc(PAID_AT), Some(PAID_AT_UNIX));
        assert_eq!(parse_iso8601_utc("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_iso8601_utc("garbage"), None);
    }
}
