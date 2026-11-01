//! Pure verdict logic for the Paystack check (no I/O here, so it is unit-tested natively).
//!
//! Paystack API: GET https://api.paystack.co/transaction/verify/:reference with
//! `Authorization: Bearer <secret key>` (https://paystack.com/docs/api/transaction/#verify).
//! Response fields used: `status` (bool), `data.status` ("success" | "abandoned" | "failed" | ...),
//! `data.reference`, `data.amount` (subunit: pesewas), `data.currency`, `data.paid_at` (ISO 8601).
//! Observed in test mode (2026-11-01): unknown reference → HTTP 400 with
//! `code: "transaction_not_found"`.
//!
//! SPEC §4 step 4 rules:
//! - PAID: success, amount ≥ expected, currency matches, reference matches, paid_at ≤ deadline.
//! - NOT_PAID: Paystack answered clearly and one of the above fails.
//! - UNVERIFIABLE: auth error, network/5xx, timeout, malformed or unclear answer.
//!   "Could not verify" is never "not paid" (fail closed).

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
pub struct Input {
    pub reference: String,
    pub expected_amount_pesewas: u64,
    pub currency: String,
    /// Unix seconds. A payment made after this is NOT_PAID.
    pub pay_deadline: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Verdict {
    Paid,
    NotPaid,
    Unverifiable,
}

/// What the program prints. Never includes the key, phone numbers or customer data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Output {
    pub verdict: Verdict,
    pub reference: String,
    /// Short machine-readable reason, e.g. "ok", "not_found", "auth_error".
    pub reason: String,
}

fn out(verdict: Verdict, input_ref: &str, reason: &str) -> Output {
    Output { verdict, reference: input_ref.to_string(), reason: reason.to_string() }
}

/// References we issue look like `opr-<order_id>`. Anything else is refused before any HTTP call,
/// so a caller cannot reach another Paystack endpoint through the URL path.
pub fn reference_is_valid(r: &str) -> bool {
    r.starts_with("opr-")
        && r.len() <= 100
        && r.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Decide from Paystack's HTTP status and body.
pub fn decide(input: &Input, http_status: u16, body: &[u8]) -> Output {
    let r = input.reference.as_str();
    match http_status {
        200 => {}
        401 | 403 => return out(Verdict::Unverifiable, r, "auth_error"),
        400 | 404 => {
            // Only a clear "transaction not found" counts as an answer.
            let not_found = serde_json::from_slice::<Value>(body)
                .ok()
                .and_then(|v| v.get("code").and_then(|c| c.as_str()).map(|c| c == "transaction_not_found"))
                .unwrap_or(false);
            return if not_found {
                out(Verdict::NotPaid, r, "not_found")
            } else {
                out(Verdict::Unverifiable, r, "unclear_4xx")
            };
        }
        s if s >= 500 => return out(Verdict::Unverifiable, r, "server_error"),
        _ => return out(Verdict::Unverifiable, r, "unexpected_status"),
    }

    let v: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => return out(Verdict::Unverifiable, r, "malformed"),
    };
    if v.get("status").and_then(|s| s.as_bool()) != Some(true) {
        return out(Verdict::Unverifiable, r, "malformed");
    }
    let d = match v.get("data") {
        Some(d) if d.is_object() => d,
        _ => return out(Verdict::Unverifiable, r, "malformed"),
    };
    let tx_status = match d.get("status").and_then(|s| s.as_str()) {
        Some(s) => s,
        None => return out(Verdict::Unverifiable, r, "malformed"),
    };
    if tx_status != "success" {
        return out(Verdict::NotPaid, r, "not_success");
    }
    if d.get("reference").and_then(|s| s.as_str()) != Some(r) {
        return out(Verdict::NotPaid, r, "reference_mismatch");
    }
    if d.get("currency").and_then(|s| s.as_str()) != Some(input.currency.as_str()) {
        return out(Verdict::NotPaid, r, "currency_mismatch");
    }
    let amount = match d.get("amount").and_then(|a| a.as_u64()) {
        Some(a) => a,
        None => return out(Verdict::Unverifiable, r, "malformed"),
    };
    if amount < input.expected_amount_pesewas {
        return out(Verdict::NotPaid, r, "amount_short");
    }
    let paid_at = match d.get("paid_at").and_then(|s| s.as_str()).and_then(parse_iso8601_utc) {
        Some(t) => t,
        None => return out(Verdict::Unverifiable, r, "malformed"),
    };
    if paid_at > input.pay_deadline {
        return out(Verdict::NotPaid, r, "late");
    }
    out(Verdict::Paid, r, "ok")
}

/// Parse `YYYY-MM-DDTHH:MM:SS[.fff]Z` to Unix seconds. Returns None for anything else.
pub fn parse_iso8601_utc(s: &str) -> Option<u64> {
    let b = s.as_bytes();
    if b.len() < 20 || *b.last()? != b'Z' {
        return None;
    }
    let num = |from: usize, to: usize| -> Option<u64> { s.get(from..to)?.parse::<u64>().ok() };
    if b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let (y, mo, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (h, mi, se) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    // Optional fraction between seconds and Z.
    let rest = &s[19..s.len() - 1];
    if !(rest.is_empty() || (rest.starts_with('.') && rest.len() > 1 && rest[1..].bytes().all(|c| c.is_ascii_digit()))) {
        return None;
    }
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || se > 59 || y < 1970 {
        return None;
    }
    // Days from civil (Howard Hinnant's algorithm).
    let (y, m) = (y as i64, mo as i64);
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    Some(days as u64 * 86400 + h * 3600 + mi * 60 + se)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAID_AT: &str = "2026-11-01T20:48:20.000Z";
    const PAID_AT_UNIX: u64 = 1_793_566_100;

    fn input() -> Input {
        Input {
            reference: "opr-test-a-paid-1".into(),
            expected_amount_pesewas: 1000,
            currency: "GHS".into(),
            pay_deadline: PAID_AT_UNIX + 600,
        }
    }

    fn body(status: &str, reference: &str, amount: u64, currency: &str, paid_at: &str) -> Vec<u8> {
        serde_json::json!({
            "status": true,
            "message": "Verification successful",
            "data": {"status": status, "reference": reference, "amount": amount,
                     "currency": currency, "paid_at": paid_at, "domain": "test"}
        })
        .to_string()
        .into_bytes()
    }

    fn ok_body() -> Vec<u8> {
        body("success", "opr-test-a-paid-1", 1000, "GHS", PAID_AT)
    }

    #[test]
    fn iso_parse() {
        assert_eq!(parse_iso8601_utc(PAID_AT), Some(PAID_AT_UNIX));
        assert_eq!(parse_iso8601_utc("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_iso8601_utc("2024-02-29T12:00:00Z"), Some(1_709_208_000));
        assert_eq!(parse_iso8601_utc("2026-11-01 20:48:20Z"), None);
        assert_eq!(parse_iso8601_utc("2026-11-01T20:48:20+01:00"), None);
        assert_eq!(parse_iso8601_utc("garbage"), None);
    }

    #[test]
    fn paid() {
        assert_eq!(decide(&input(), 200, &ok_body()).verdict, Verdict::Paid);
    }

    #[test]
    fn overpaid_is_paid() {
        let b = body("success", "opr-test-a-paid-1", 1500, "GHS", PAID_AT);
        assert_eq!(decide(&input(), 200, &b).verdict, Verdict::Paid);
    }

    #[test]
    fn not_found_is_not_paid() {
        let b = br#"{"status":false,"message":"Transaction reference not found.","code":"transaction_not_found"}"#;
        assert_eq!(decide(&input(), 400, b), out(Verdict::NotPaid, "opr-test-a-paid-1", "not_found"));
    }

    #[test]
    fn other_400_is_unverifiable() {
        let b = br#"{"status":false,"message":"Something else"}"#;
        assert_eq!(decide(&input(), 400, b).verdict, Verdict::Unverifiable);
    }

    #[test]
    fn abandoned_is_not_paid() {
        let b = body("abandoned", "opr-test-a-paid-1", 1000, "GHS", PAID_AT);
        assert_eq!(decide(&input(), 200, &b).reason, "not_success");
    }

    #[test]
    fn short_amount_is_not_paid() {
        let b = body("success", "opr-test-a-paid-1", 999, "GHS", PAID_AT);
        assert_eq!(decide(&input(), 200, &b).reason, "amount_short");
    }

    #[test]
    fn wrong_currency_is_not_paid() {
        let b = body("success", "opr-test-a-paid-1", 1000, "NGN", PAID_AT);
        assert_eq!(decide(&input(), 200, &b).reason, "currency_mismatch");
    }

    #[test]
    fn other_reference_is_not_paid() {
        let b = body("success", "opr-other", 1000, "GHS", PAID_AT);
        assert_eq!(decide(&input(), 200, &b).reason, "reference_mismatch");
    }

    #[test]
    fn late_is_not_paid() {
        let mut i = input();
        i.pay_deadline = PAID_AT_UNIX - 1;
        assert_eq!(decide(&i, 200, &ok_body()).reason, "late");
    }

    #[test]
    fn paid_exactly_at_deadline_is_paid() {
        let mut i = input();
        i.pay_deadline = PAID_AT_UNIX;
        assert_eq!(decide(&i, 200, &ok_body()).verdict, Verdict::Paid);
    }

    #[test]
    fn auth_errors_are_unverifiable_never_not_paid() {
        for s in [401, 403] {
            assert_eq!(decide(&input(), s, b"{}").verdict, Verdict::Unverifiable);
        }
    }

    #[test]
    fn server_errors_are_unverifiable() {
        for s in [500, 502, 503, 504] {
            assert_eq!(decide(&input(), s, b"").verdict, Verdict::Unverifiable);
        }
        assert_eq!(decide(&input(), 429, b"").verdict, Verdict::Unverifiable);
    }

    #[test]
    fn malformed_200_is_unverifiable() {
        assert_eq!(decide(&input(), 200, b"not json").verdict, Verdict::Unverifiable);
        assert_eq!(decide(&input(), 200, br#"{"status":true}"#).verdict, Verdict::Unverifiable);
        assert_eq!(decide(&input(), 200, br#"{"status":false,"data":{}}"#).verdict, Verdict::Unverifiable);
        let no_paid_at = body("success", "opr-test-a-paid-1", 1000, "GHS", "");
        assert_eq!(decide(&input(), 200, &no_paid_at).verdict, Verdict::Unverifiable);
    }

    #[test]
    fn reference_validation() {
        assert!(reference_is_valid("opr-42"));
        assert!(reference_is_valid("opr-test-a-paid-1"));
        assert!(!reference_is_valid("42"));
        assert!(!reference_is_valid("opr-1/../../transfer"));
        assert!(!reference_is_valid("opr-1?x=1"));
        assert!(!reference_is_valid("opr-1%2F"));
        assert!(!reference_is_valid(&format!("opr-{}", "a".repeat(200))));
    }
}
