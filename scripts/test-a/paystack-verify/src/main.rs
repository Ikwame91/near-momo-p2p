//! OutLayer program: reads `Input` JSON from stdin, checks the payment on Paystack with the key
//! from the OutLayer secret env var `PAYSTACK_SECRET_KEY`, prints one `Output` JSON line.
//! Every failure path prints UNVERIFIABLE (fail closed); it never prints the key or the
//! Paystack response body (which contains customer data).

#[cfg(target_arch = "wasm32")]
use paystack_verify::decide;
use paystack_verify::{reference_is_valid, Input, Output, Verdict};
use std::io::Read;

const VERIFY_URL: &str = "https://api.paystack.co/transaction/verify/";

fn print(o: &Output) {
    println!("{}", serde_json::to_string(o).expect("serialize output"));
}

fn unverifiable(reference: &str, reason: &str) -> Output {
    Output { verdict: Verdict::Unverifiable, reference: reference.to_string(), reason: reason.to_string() }
}

fn main() {
    let mut buf = String::new();
    if std::io::stdin().read_to_string(&mut buf).is_err() {
        return print(&unverifiable("", "bad_input"));
    }
    let input: Input = match serde_json::from_str(&buf) {
        Ok(i) => i,
        Err(_) => return print(&unverifiable("", "bad_input")),
    };
    if !reference_is_valid(&input.reference) {
        return print(&unverifiable(&input.reference, "bad_reference"));
    }
    let key = match std::env::var("PAYSTACK_SECRET_KEY") {
        Ok(k) if !k.trim().is_empty() => k,
        _ => return print(&unverifiable(&input.reference, "no_key")),
    };
    print(&check(&input, key.trim()));
}

#[cfg(target_arch = "wasm32")]
fn check(input: &Input, key: &str) -> Output {
    use std::time::Duration;
    let url = format!("{VERIFY_URL}{}", input.reference);
    let resp = wasi_http_client::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {key}"))
        .header("User-Agent", "openramp-verifier/0.1")
        .connect_timeout(Duration::from_secs(10))
        .send();
    let resp = match resp {
        Ok(r) => r,
        Err(_) => return unverifiable(&input.reference, "network_error"),
    };
    let status = resp.status();
    match resp.body() {
        Ok(body) => decide(input, status, &body),
        Err(_) => unverifiable(&input.reference, "network_error"),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn check(input: &Input, _key: &str) -> Output {
    // Native builds exist only for unit tests of the library; HTTP runs inside OutLayer.
    let _ = VERIFY_URL;
    unverifiable(&input.reference, "native_build_no_http")
}
