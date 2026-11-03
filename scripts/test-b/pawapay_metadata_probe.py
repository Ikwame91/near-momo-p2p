#!/usr/bin/env python3
"""Q5 probe (D-018): does pawaPay return isPII metadata (the sell-order salt) in clear in the payout
status reply? Sandbox only; phone number masked in output. Docs:
https://docs.pawapay.io/v2/api-reference/payouts/initiate-payout (metadata),
https://docs.pawapay.io/v2/api-reference/payouts/check-payout-status.
Usage: python3 scripts/test-b/pawapay_metadata_probe.py (proxy or PAWAPAY_API_TOKEN supplies auth)."""

import json, os, re, secrets, time, urllib.request, uuid
BASE = "https://api.sandbox.pawapay.io"
def call(m, p, b=None):
    r = urllib.request.Request(BASE + p, data=json.dumps(b).encode() if b else None,
                               headers={"Content-Type": "application/json",
                                        **({"Authorization": "Bearer " + os.environ["PAWAPAY_API_TOKEN"]}
                                           if os.environ.get("PAWAPAY_API_TOKEN") else {})}, method=m)
    with urllib.request.urlopen(r, timeout=30) as x:
        return x.status, x.read().decode()
salt = secrets.token_hex(32)                      # 32 bytes = 64 hex chars, fresh per order
pid = str(uuid.uuid4())
body = {"payoutId": pid, "amount": "5", "currency": "GHS",
        "recipient": {"type": "MMO", "accountDetails": {"phoneNumber": "233593456789", "provider": "MTN_MOMO_GHA"}},
        "metadata": [{"openrampSalt": salt, "isPII": True}]}
print("SENT salt:", salt)
s, out = call("POST", "/v2/payouts", body); print("INITIATE", s, out)
for _ in range(20):
    time.sleep(3)
    s, raw = call("GET", f"/v2/payouts/{pid}")
    if '"COMPLETED"' in raw or '"FAILED"' in raw: break
masked = re.sub(r'"phoneNumber":"(\d+)(\d{3})"', lambda m: '"phoneNumber":"' + "*" * len(m.group(1)) + m.group(2) + '"', raw)
print("STATUS", s, "RAW (phone masked):")
print(json.dumps(json.loads(masked), indent=2))
print("salt returned in clear and identical:", salt in raw)
