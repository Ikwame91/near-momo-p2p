#!/usr/bin/env python3
"""Test B (PLAN M0.6): Paystack TEST MODE mobile money charge + verify, transfer + verify (GHS).

Usage: python3 scripts/test-b/run.py
Auth: PAYSTACK_SECRET_KEY (sk_test_...) if set; in cloud sessions the agent proxy adds it.
Refuses to run unless Paystack reports domain == "test". Phone numbers are masked in output.
Exit codes: 0 all steps passed, 1 a step failed, 2 blocked by the Paystack account (transfers).

Paystack API (https://paystack.com/docs/api/):
  POST /transaction/initialize, GET /transaction/verify/:ref   (preflight: test mode check)
  POST /charge with mobile_money {phone, provider}              (charge)
  POST /transferrecipient type=mobile_money, bank_code MTN|VOD|ATL (GET /bank?currency=GHS&type=mobile_money)
  POST /transfer, GET /transfer/verify/:ref                     (payout)
"""
import json
import os
import sys
import time
import urllib.error
import urllib.request

API = "https://api.paystack.co"
# Paystack's public test number for MTN Ghana mobile money (test mode only).
TEST_PHONE = "0551234987"
TEST_PROVIDER = "mtn"          # charge API provider code
TEST_BANK_CODE = "MTN"         # transfer recipient bank code
AMOUNT_CHARGE = 1000           # pesewas = GHS 10.00
AMOUNT_PAYOUT = 500            # pesewas = GHS 5.00


def mask(phone):
    return "***" + str(phone)[-3:] if phone else ""


def call(method, path, body=None):
    """Return (http_status, parsed_json). Network errors raise: fail loudly, never guess."""
    headers = {"Content-Type": "application/json", "User-Agent": "openramp-test-b/0.1"}
    key = os.environ.get("PAYSTACK_SECRET_KEY", "").strip()
    if key:
        if not key.startswith("sk_test_"):
            sys.exit("PAYSTACK_SECRET_KEY is not a test key (sk_test_...). Refusing to run.")
        headers["Authorization"] = "Bearer " + key
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(API + path, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read() or b"{}")


def step(name, ok, detail):
    print(("PASS " if ok else "FAIL ") + name + ": " + detail)
    return ok


def main():
    run = str(int(time.time()))
    ok = True

    # 0. Preflight: prove test mode before anything else.
    ref0 = f"opr-test-b-mode-{run}"
    s, d = call("POST", "/transaction/initialize",
                {"email": "buyer@example.com", "amount": 100, "currency": "GHS", "reference": ref0})
    if s != 200:
        sys.exit(f"preflight initialize failed: HTTP {s} {d.get('message')}")
    s, d = call("GET", f"/transaction/verify/{ref0}")
    domain = (d.get("data") or {}).get("domain")
    if domain != "test":
        sys.exit(f"preflight: Paystack domain is {domain!r}, not 'test'. Refusing to run.")
    print(f"PASS preflight: domain=test (ref {ref0})")

    # 1. Mobile money charge with our own reference, then verify it.
    ref1 = f"opr-test-b-charge-{run}"
    s, d = call("POST", "/charge", {"email": "buyer@example.com", "amount": AMOUNT_CHARGE, "currency": "GHS",
                                    "reference": ref1,
                                    "mobile_money": {"phone": TEST_PHONE, "provider": TEST_PROVIDER}})
    t = d.get("data") or {}
    ok &= step("charge", s == 200 and t.get("status") == "success",
               f"HTTP {s}, status={t.get('status')}, gateway={t.get('gateway_response')}, phone {mask(TEST_PHONE)}")
    s, d = call("GET", f"/transaction/verify/{ref1}")
    t = d.get("data") or {}
    ok &= step("charge verify", s == 200 and t.get("status") == "success" and t.get("reference") == ref1
               and t.get("amount") == AMOUNT_CHARGE and t.get("currency") == "GHS",
               f"status={t.get('status')}, ref={t.get('reference')}, amount={t.get('amount')} {t.get('currency')}, "
               f"channel={t.get('channel')}, paid_at={t.get('paid_at')}")

    # 2. Transfer recipient (mobile money).
    s, d = call("POST", "/transferrecipient", {"type": "mobile_money", "name": "Test Seller",
                                               "account_number": TEST_PHONE, "bank_code": TEST_BANK_CODE,
                                               "currency": "GHS"})
    t = d.get("data") or {}
    rcp = t.get("recipient_code")
    det = t.get("details") or {}
    ok &= step("recipient", s in (200, 201) and bool(rcp),
               f"HTTP {s}, {rcp}, number {mask(det.get('account_number'))}, bank {det.get('bank_code')}")
    if not rcp:
        return 1

    # 3. Transfer (payout) with our own reference.
    ref3 = f"opr-test-b-payout-{run}"
    s, d = call("POST", "/transfer", {"source": "balance", "amount": AMOUNT_PAYOUT, "recipient": rcp,
                                      "reference": ref3, "reason": "OpenRamp Test B", "currency": "GHS"})
    if s != 200:
        # Only the Starter-account restriction counts as "blocked" (exit 2). Any other error
        # (insufficient balance, invalid recipient, server error) is a real failure (exit 1).
        # https://paystack.com/docs/api/errors/transfer/
        blocked = d.get("code") == "transfer_unavailable"
        label = "BLOCKED" if blocked else "FAIL"
        print(f"{label} transfer: HTTP {s}, code={d.get('code')}, message={d.get('message')!r}")
        s, d = call("GET", f"/transfer/verify/{ref3}")
        print(f"INFO transfer verify (no transfer): HTTP {s}, code={d.get('code')}, message={d.get('message')!r}")
        return 2 if (ok and blocked) else 1
    t = d.get("data") or {}
    step("transfer", True, f"status={t.get('status')}, ref={t.get('reference')}, code={t.get('transfer_code')}")

    # 4. Transfer verify, including who received it (needed for SPEC inv. 13 / T23).
    time.sleep(5)
    s, d = call("GET", f"/transfer/verify/{ref3}")
    t = d.get("data") or {}
    r = t.get("recipient") if isinstance(t.get("recipient"), dict) else {}
    rdet = (r or {}).get("details") or {}
    ok &= step("transfer verify", s == 200 and t.get("reference") == ref3 and t.get("amount") == AMOUNT_PAYOUT,
               f"status={t.get('status')}, amount={t.get('amount')} {t.get('currency')}, "
               f"recipient number {mask(rdet.get('account_number'))}, bank {rdet.get('bank_code')}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
