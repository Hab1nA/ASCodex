#!/usr/bin/env python3
"""Verify every token in the Downloads token file against /api/auth/me,
print id/name/userType/agentFramework, and detect any human token.
Also saves a machine-readable map (name -> token) to _logs for reuse."""
import json
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
BASE = "https://play.bohrium.com/api"
SRC = r"C:\Users\XKZ\Downloads\agent token.txt"

rows = []
for line in open(SRC, encoding="utf-8", errors="replace"):
    line = line.strip()
    if not line:
        continue
    parts = line.split()
    if len(parts) < 2:
        print(f"[skip malformed] {line[:60]!r}")
        continue
    name, tok = parts[0], parts[1]
    try:
        r = requests.get(f"{BASE}/auth/me", headers={"Authorization": f"Bearer {tok}"}, timeout=30)
        if r.status_code != 200:
            print(f"{name:<28} -> HTTP {r.status_code} {r.text[:80]!r}")
            rows.append({"display": name, "id": None, "name": None, "userType": None,
                         "framework": None, "token": tok, "valid": False})
            continue
        d = r.json()
        rows.append({
            "display": name,
            "id": d.get("id"),
            "name": d.get("name"),
            "userType": d.get("userType"),
            "framework": d.get("agentFramework"),
            "token": tok,
            "valid": True,
        })
        print(f"{name:<28} -> id={d.get('id'):<28} userType={str(d.get('userType')):<6} "
              f"framework={d.get('agentFramework')}")
    except Exception as ex:
        print(f"{name:<28} -> ERR {ex}")

os.makedirs("_logs", exist_ok=True)
with open("_logs/agent_tokens_2026-08-21.json", "w", encoding="utf-8") as f:
    json.dump(rows, f, ensure_ascii=False, indent=1)
humans = [r for r in rows if r.get("userType") == "human"]
print(f"\ntotal={len(rows)} valid={sum(1 for r in rows if r.get('valid'))} "
      f"human={len(humans)} agents={sum(1 for r in rows if r.get('userType') == 'agent')}")
print("saved _logs/agent_tokens_2026-08-21.json")
