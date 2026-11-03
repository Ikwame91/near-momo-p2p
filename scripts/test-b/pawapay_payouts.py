#!/usr/bin/env python3
"""Test B2 (PLAN M0.6b, DECISIONS D-011): pawaPay SANDBOX mobile money payouts in Ghana.

Usage: python3 scripts/test-b/pawapay_payouts.py
Auth: PAWAPAY_API_TOKEN if set; in cloud sessions the agent proxy adds the token.
Refuses any base URL other than the sandbox. Phone numbers are masked in output (they are
pawaPay's public test numbers, but the rule is: no phone numbers in logs).
Exit codes: 0 all checks passed, 1 a check failed.

pawaPay API v2 (https://docs.pawapay.io/v2/api-reference/):
  GET  /v2/active-conf               country/provider configuration
  GET  /v2/wallet-balances           sandbox wallet balances
  POST /v2/payouts                   initiate payout (payoutId = UUID we choose; idempotent)
  GET  /v2/payouts/{payoutId}        payout status, incl. recipient phone + provider
Test numbers (https://docs.pawapay.io/v2/docs/test_numbers), MTN_MOMO_GHA payouts:
  233593456789 COMPLETED · 233593456089 FAILED RECIPIENT_NOT_FOUND · 233593456129 SUBMITTED (pending)
"""
import json
import os
import sys
import time
import urllib.error
import urllib.request
import uuid

BASE = os.environ.get("PAWAPAY_BASE_URL", "https://api.sandbox.pawapay.io").rstrip("/")
PROVIDER = "MTN_MOMO_GHA"
CURRENCY = "GHS"
AMOUNT = "5"
CASES = [  # (name, test number, expected final status, expected failure code)
    ("completed", "233593456789", "COMPLETED", None),
    ("failed", "233593456089", "FAILED", "RECIPIENT_NOT_FOUND"),
    ("pending", "233593456129", None, None),  # stays non-final in the sandbox
]
FINAL = {"COMPLETED", "FAILED"}
POLL_SECONDS = 60


def mask(phone):
    return "***" + str(phone)[-3:] if phone else ""


def call(method, path, body=None):
    """Return (http_status, parsed JSON or {}). Network errors raise: fail loudly."""
    headers = {"Content-Type": "application/json", "User-Agent": "openramp-test-b2/0.1"}
    token = os.environ.get("PAWAPAY_API_TOKEN", "").strip()
    if token:
        headers["Authorization"] = "Bearer " + token
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(BASE + path, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            raw = r.read()
            return r.status, (json.loads(raw) if raw else {})
    except urllib.error.HTTPError as e:
        raw = e.read()
        try:
            return e.code, (json.loads(raw) if raw else {})
        except ValueError:
            return e.code, {"_raw": raw[:200].decode(errors="replace")}


def check(name, ok, detail):
    print(("PASS " if ok else "FAIL ") + name + ": " + detail)
    return ok


def ghs_balance():
    s, d = call("GET", "/v2/wallet-balances")
    for b in (d.get("balances") or []) if s == 200 else []:
        if b.get("country") == "GHA" and b.get("currency") == CURRENCY:
            return b.get("balance")
    return None


def get_payout(pid):
    s, d = call("GET", f"/v2/payouts/{pid}")
    return s, d.get("status"), (d.get("data") or {})


def main():
    if BASE != "https://api.sandbox.pawapay.io":
        sys.exit(f"Refusing to run against {BASE}: sandbox only (D-011).")
    ok = True

    # 0. Configuration: Ghana provider enabled for payouts, and the wallet has funds.
    s, conf = call("GET", "/v2/active-conf")
    providers = {p["provider"]: p for c in conf.get("countries", []) if c.get("country") == "GHA"
                 for p in c.get("providers", [])}
    payout_status = None
    for cur in (providers.get(PROVIDER) or {}).get("currencies", []):
        if cur.get("currency") == CURRENCY:
            payout_status = ((cur.get("operationTypes") or {}).get("PAYOUT") or {}).get("status")
    ok &= check("config", s == 200 and payout_status == "OPERATIONAL",
                f"HTTP {s}, {PROVIDER} {CURRENCY} PAYOUT={payout_status}, GHA providers={sorted(providers)}")
    before = ghs_balance()
    ok &= check("wallet", before is not None and float(before) >= float(AMOUNT) * len(CASES),
                f"GHA {CURRENCY} balance before = {before}")
    if not ok:
        return 1

    # 1. Initiate one payout per test number.
    ids = {}
    for name, phone, _, _ in CASES:
        pid = str(uuid.uuid4())
        body = {"payoutId": pid, "amount": AMOUNT, "currency": CURRENCY,
                "recipient": {"type": "MMO", "accountDetails": {"phoneNumber": phone, "provider": PROVIDER}},
                "customerMessage": "OpenRamp Test B2"}
        s, d = call("POST", "/v2/payouts", body)
        ids[name] = pid
        ok &= check(f"initiate {name}", s == 200 and d.get("status") == "ACCEPTED",
                    f"HTTP {s}, status={d.get('status')}, payoutId={pid}, to {mask(phone)}, "
                    f"failure={d.get('failureReason')}")

    # 2. Replay protection: the same payoutId again must be ignored.
    first = CASES[0]
    s, d = call("POST", "/v2/payouts", {"payoutId": ids["completed"], "amount": AMOUNT, "currency": CURRENCY,
                                        "recipient": {"type": "MMO", "accountDetails":
                                                      {"phoneNumber": first[1], "provider": PROVIDER}}})
    ok &= check("duplicate payoutId", d.get("status") == "DUPLICATE_IGNORED",
                f"HTTP {s}, status={d.get('status')}")

    # 3. Poll each payout until final (or timeout for the pending case), then check the details
    #    the verifier needs: status, amount, currency, recipient phone + provider (SPEC §5, T23).
    deadline = time.time() + POLL_SECONDS
    results = {}
    while time.time() < deadline:
        for name, *_ in CASES:
            if name not in results or results[name][1].get("status") not in FINAL:
                results[name] = get_payout(ids[name])[1:]
        if all(results[n][1].get("status") in FINAL for n, _, exp, _ in CASES if exp):
            break
        time.sleep(3)
    for name, phone, exp_status, exp_code in CASES:
        found, data = results[name]
        acct = (data.get("recipient") or {}).get("accountDetails") or {}
        status = data.get("status")
        failure = (data.get("failureReason") or {}).get("failureCode")
        recipient_ok = acct.get("phoneNumber") == phone and acct.get("provider") == PROVIDER
        detail = (f"found={found}, status={status}, failure={failure}, amount={data.get('amount')} "
                  f"{data.get('currency')}, recipient {mask(acct.get('phoneNumber'))} {acct.get('provider')} "
                  f"(matches sent: {recipient_ok})")
        if exp_status:
            good = (found == "FOUND" and status == exp_status and failure == exp_code and recipient_ok
                    and data.get("currency") == CURRENCY and float(data.get("amount", "nan")) == float(AMOUNT))
        else:
            good = found == "FOUND" and status not in FINAL and recipient_ok
        ok &= check(f"status {name}", good, detail)

    # 4. An id we never used.
    s, found, _ = get_payout(str(uuid.uuid4()))
    ok &= check("status unknown id", s == 200 and found == "NOT_FOUND", f"HTTP {s}, status={found}")

    after = ghs_balance()
    print(f"INFO wallet GHA {CURRENCY}: before={before}, after={after}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
