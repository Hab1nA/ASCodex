#!/usr/bin/env python3
"""Batch rename agent accounts per user plan:
  - friday (retired main account) -> Demon
  - the 24 immediately-usable accounts (A-tier 25 minus friday), sorted by id,
    split into 3 groups of 8: Friday-01..08 / Jarvis-01..08 / Ultron-01..08
  - everything else unchanged.

Usage: python rename_agents.py [--execute]   (dry-run by default)
"""
import json
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
BASE = "https://play.bohrium.com/api"

RETIRE = {"friday": "Demon"}
USABLE_24 = [
    "friday-s2-24714", "friday-s3-67618", "friday-t51795", "friday-u1",
    "friday-r1", "friday-r2", "friday-r3", "friday-u2", "friday-u2-51065",
    "friday-u3", "friday-u3-51065", "friday-u4-52367", "friday-u5-52903",
    "friday-u6-53704", "friday-u7-54212", "friday-n55379-n1",
    "friday-n55379-n2", "friday-n55379-n3", "friday-p51288",
    "jarvis", "jarvis-2", "jarvis-3", "jarvis-4", "ultron",
]
GROUP_PREFIXES = ["Friday", "Jarvis", "Ultron"]


def read_human_token():
    p = r"C:\Users\XKZ\.dsh\human_credentials.txt"
    m = re.search(r"(?m)^\s*api_token\s*=\s*(\S+)", open(p, encoding="utf-8").read())
    return m.group(1)


def plan():
    mapping = dict(RETIRE)
    ids = sorted(USABLE_24)
    per = len(ids) // len(GROUP_PREFIXES)
    for gi, prefix in enumerate(GROUP_PREFIXES):
        chunk = ids[gi * per:(gi + 1) * per]
        for i, aid in enumerate(chunk, start=1):
            mapping[aid] = f"{prefix}-{i:02d}"
    return mapping


def main():
    execute = "--execute" in sys.argv
    mapping = plan()
    tok = read_human_token()
    H = {"Authorization": f"Bearer {tok}"}

    print("=== rename plan ===")
    for aid, new in sorted(mapping.items()):
        print(f"  {aid:<32} -> {new}")

    # sanity: no duplicate target names
    targets = list(mapping.values())
    assert len(targets) == len(set(targets)), "duplicate target names!"

    if not execute:
        print("\n[dry-run] pass --execute to apply")
        return

    print("\n=== applying ===")
    results = {}
    for aid, new in mapping.items():
        r = requests.patch(f"{BASE}/agent/register/{aid}",
                           headers={**H, "Content-Type": "application/json"},
                           data=json.dumps({"name": new}), timeout=60)
        results[aid] = (new, r.status_code, r.text[:80])
        print(f"  PATCH {aid:<32} -> {new:<14} {r.status_code}")

    fails = [(a, v) for a, v in results.items() if v[1] != 200]
    print(f"\nok={len(results) - len(fails)} fail={len(fails)}")
    for a, v in fails:
        print(f"  FAIL {a}: {v}")

    # verify via /api/users
    print("\n=== verify ===")
    users = requests.get(f"{BASE}/users", timeout=120).json()
    for aid, new in mapping.items():
        for u in users.values():
            if u.get("id") == aid:
                ok = (u.get("name") or "") == new
                print(f"  {aid:<32} name={u.get('name')!r} {'OK' if ok else 'MISMATCH'}")
                break

    # save mapping for the knowledge base
    with open("_logs/rename_map_2026-08-21.json", "w", encoding="utf-8") as f:
        json.dump(mapping, f, ensure_ascii=False, indent=1)
    print("\nsaved _logs/rename_map_2026-08-21.json")


if __name__ == "__main__":
    main()
