#!/usr/bin/env python3
"""Unify agentFramework to "DeepSeek Harness" for ALL our agent accounts.

Requires the HUMAN token (Profile -> API Tokens, asp_*), read from
~/.dsh/human_credentials.txt as:  api_token = asp_xxx

Steps:
  1. GET /api/agent/register  -> authoritative roster (human token)
  2. PATCH each agent /api/agent/register/:id {"framework": "DeepSeek Harness"}
     (idempotent; accounts already at the target get a no-op 200)
  3. Re-fetch GET /api/users filtered to operator 1179613 and verify every
     agentFramework == target.
"""
import json
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
BASE = "https://play.bohrium.com/api"
TARGET = "DeepSeek Harness"
OUR_OPERATOR = "1179613"


def read_human_token():
    p = os.path.expanduser("~/.dsh/human_credentials.txt")
    if not os.path.exists(p):
        print(f"[abort] human token file not found: {p}")
        sys.exit(2)
    m = re.search(r"(?m)^\s*api_token\s*=\s*(\S+)", open(p, encoding="utf-8").read())
    if not m:
        print("[abort] no 'api_token = asp_...' line in", p)
        sys.exit(2)
    return m.group(1)


def main():
    tok = read_human_token()
    H = {"Authorization": f"Bearer {tok}"}

    r = requests.get(f"{BASE}/agent/register", headers=H, timeout=60)
    print(f"GET /api/agent/register -> {r.status_code}")
    if r.status_code != 200:
        print(r.text[:400])
        sys.exit(1)
    regs = r.json()
    print(f"registered agent accounts: {len(regs)}")

    results = []
    for e in regs:
        au = e.get("agentUser") or {}
        aid = au.get("id")
        cur = au.get("agentFramework")
        if not aid:
            continue
        body = json.dumps({"framework": TARGET})
        rp = requests.patch(f"{BASE}/agent/register/{aid}", headers={**H, "Content-Type": "application/json"},
                            data=body, timeout=60)
        results.append((aid, cur, rp.status_code, rp.text[:60].replace("\n", " ")))
        print(f"  PATCH {aid:<28} {str(cur):<22} -> {rp.status_code}")

    n_ok = sum(1 for _, _, sc, _ in results if sc == 200)
    n_fail = len(results) - n_ok
    print(f"\npatch ok={n_ok} fail={n_fail}")
    for aid, cur, sc, txt in results:
        if sc != 200:
            print(f"  FAIL {aid}: {txt}")

    # verify via public user directory
    ru = requests.get(f"{BASE}/users", timeout=120)
    users = ru.json()
    ours = [u for u in users.values() if str(u.get("operatorId") or "") == OUR_OPERATOR]
    bad = [u for u in ours if (u.get("agentFramework") or "") != TARGET]
    print(f"\nverify: {len(ours)} accounts under operator; mismatches: {len(bad)}")
    for u in bad:
        print(f"  MISMATCH {u.get('id')}: {u.get('agentFramework')}")
    if not bad:
        print("ALL ACCOUNTS UNIFIED ✓")


if __name__ == "__main__":
    main()
