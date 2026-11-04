//! OutLayer program entry: reads one `Input` JSON from stdin, uses the LP's provider key from the
//! OutLayer secret env var `PAYSTACK_SECRET_KEY`, prints one JSON line. Every failure path prints a
//! fail-closed answer; it never prints the key or Paystack's response body (customer data).
//! Paystack: https://paystack.com/docs/api/transaction/

#[cfg(target_arch = "wasm32")]
use openramp_verifier::{checkout_result, decide, CHECKOUT_EMAIL, INITIALIZE_URL, VERIFY_URL};
use openramp_verifier::{key_fp, reference_is_valid, InfoOutput, Input, Verdict, VerifyOutput};
use std::io::Read;

fn print<T: serde::Serialize>(o: &T) {
    println!("{}", serde_json::to_string(o).expect("serialize output"));
}

fn unverifiable(reference: &str, fp: &str, reason: &str) -> VerifyOutput {
    VerifyOutput { verdict: Verdict::Unverifiable, reference: reference.into(), key_fp: fp.into(), reason: reason.into() }
}

fn main() {
    let mut buf = String::new();
    if std::io::stdin().read_to_string(&mut buf).is_err() {
        return print(&InfoOutput { error: Some("bad_input".into()), ..Default::default() });
    }
    let input: Input = match serde_json::from_str(&buf) {
        Ok(i) => i,
        Err(_) => return print(&InfoOutput { error: Some("bad_input".into()), ..Default::default() }),
    };
    let key = std::env::var("PAYSTACK_SECRET_KEY").ok().map(|k| k.trim().to_string()).filter(|k| !k.is_empty());
    match input {
        Input::Fingerprint => match key {
            Some(k) => print(&InfoOutput { key_fp: Some(key_fp(&k)), ..Default::default() }),
            None => print(&InfoOutput { error: Some("no_key".into()), ..Default::default() }),
        },
        Input::Checkout { reference, amount_pesewas, currency } => {
            let Some(k) = key else {
                return print(&InfoOutput { reference: Some(reference), error: Some("no_key".into()), ..Default::default() });
            };
            if !reference_is_valid(&reference) || currency != "GHS" || amount_pesewas == 0 {
                return print(&InfoOutput { key_fp: Some(key_fp(&k)), reference: Some(reference), error: Some("bad_input".into()), ..Default::default() });
            }
            print(&checkout(&reference, amount_pesewas, &k));
        }
        Input::Verify { reference, expected_amount_pesewas, currency, pay_deadline } => {
            let Some(k) = key else { return print(&unverifiable(&reference, "", "no_key")) };
            let fp = key_fp(&k);
            if !reference_is_valid(&reference) {
                return print(&unverifiable(&reference, &fp, "bad_reference"));
            }
            print(&verify(&reference, expected_amount_pesewas, &currency, pay_deadline, &k, &fp));
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn verify(reference: &str, expected: u64, currency: &str, deadline: u64, key: &str, fp: &str) -> VerifyOutput {
    use std::time::Duration;
    let resp = wasi_http_client::Client::new()
        .get(&format!("{VERIFY_URL}{reference}"))
        .header("Authorization", format!("Bearer {key}"))
        .header("User-Agent", "openramp-verifier/0.2")
        .connect_timeout(Duration::from_secs(10))
        .send();
    match resp {
        Ok(r) => {
            let status = r.status();
            match r.body() {
                Ok(body) => decide(reference, expected, currency, deadline, fp, status, &body),
                Err(_) => unverifiable(reference, fp, "network_error"),
            }
        }
        Err(_) => unverifiable(reference, fp, "network_error"),
    }
}

#[cfg(target_arch = "wasm32")]
fn checkout(reference: &str, amount: u64, key: &str) -> InfoOutput {
    use std::time::Duration;
    let fp = key_fp(key);
    let body = serde_json::json!({
        "email": CHECKOUT_EMAIL,
        "amount": amount,
        "currency": "GHS",
        "reference": reference,
        "channels": ["mobile_money"],
    });
    let resp = wasi_http_client::Client::new()
        .post(INITIALIZE_URL)
        .header("Authorization", format!("Bearer {key}"))
        .header("Content-Type", "application/json")
        .header("User-Agent", "openramp-verifier/0.2")
        .connect_timeout(Duration::from_secs(10))
        .body(body.to_string().as_bytes())
        .send();
    match resp {
        Ok(r) => {
            let status = r.status();
            match r.body() {
                Ok(b) => checkout_result(reference, &fp, status, &b),
                Err(_) => InfoOutput { key_fp: Some(fp), reference: Some(reference.into()), error: Some("network_error".into()), ..Default::default() },
            }
        }
        Err(_) => InfoOutput { key_fp: Some(fp), reference: Some(reference.into()), error: Some("network_error".into()), ..Default::default() },
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn verify(reference: &str, _e: u64, _c: &str, _d: u64, _k: &str, fp: &str) -> VerifyOutput {
    unverifiable(reference, fp, "native_build_no_http")
}

#[cfg(not(target_arch = "wasm32"))]
fn checkout(reference: &str, _a: u64, key: &str) -> InfoOutput {
    InfoOutput { key_fp: Some(key_fp(key)), reference: Some(reference.into()), error: Some("native_build_no_http".into()), ..Default::default() }
}
