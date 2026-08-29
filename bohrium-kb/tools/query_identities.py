#!/usr/bin/env python3
"""Read-only: list ALL our agent identities on play.bohrium.com and their
declared framework / model fields.

1) GET /api/agent/register with the HUMAN token -> authoritative roster
2) GET /api/agent/pending-claims                     -> unclaimed agents
3) For every local credential file: GET /api/auth/me -> per-agent declared fields
"""
import glob
import json
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
BASE = "https://play.bohrium.com/api"
DSH = os.path.expanduser("~/.dsh")


def H(tok):
    return {"Authorization": f"Bearer {tok}"}


def mask(tok):
    if not tok:
        return ""
    return tok[:12] + "…" if len(tok) > 12 else "…"


def read_human_token():
    p = os.path.expanduser("~/.config/playground/credentials.env")
    m = re.search(r"PLAYGROUND_TOKEN=(\S+)", open(p, encoding="utf-8").read())
    return m.group(1) if m else None


def list_local_credentials():
    out = []
    for f in sorted(glob.glob(os.path.join(DSH, "*credential*.txt"))):
        txt = open(f, encoding="utf-8", errors="replace").read()
        rec = {
            "file": os.path.basename(f),
            "name": (re.search(r"(?m)^\s*name\s*=\s*(\S+)", txt) or [None, ""])[1],
            "email": (re.search(r"(?m)^\s*email\s*=\s*(\S+)", txt) or [None, ""])[1],
            "api_token": (re.search(r"(?m)^\s*api_token\s*=\s*(\S+)", txt) or [None, ""])[1],
        }
        out.append(rec)
    # playground CLI agent env files
    for f in sorted(glob.glob(os.path.expanduser("~/.config/playground/agents/*.env"))):
        txt = open(f, encoding="utf-8", errors="replace").read()
        rec = {
            "file": os.path.basename(f),
            "name": (re.search(r"(?m)^\s*name\s*=\s*(\S+)", txt) or [None, ""])[1],
            "email": (re.search(r"(?m)^\s*email\s*=\s*(\S+)", txt) or [None, ""])[1],
            "api_token": (re.search(r"(?m)^\s*api_token\s*=\s*(\S+)", txt) or [None, ""])[1],
        }
        out.append(rec)
    return out


def main():
    human = read_human_token()
    print(f"human token: {'found ' + mask(human) if human else 'NOT FOUND'}")
    print("=" * 100)

    # --- authoritative roster from platform ---
    if human:
        r = requests.get(f"{BASE}/agent/register", headers=H(human), timeout=60)
        print(f"GET /api/agent/register -> {r.status_code}")
        if r.status_code == 200:
            regs = r.json()
            print(f"registered agent accounts on platform: {len(regs)}")
            for e in regs:
                au = e.get("agentUser") or {}
                print("  ROSTER", json.dumps({
                    "id": au.get("id"),
                    "name": au.get("name"),
                    "framework": au.get("agentFramework"),
                    "status": au.get("status"),
                    "operatorId": au.get("operatorId"),
                    "operatorName": au.get("operatorName"),
                    "extra": {k: v for k, v in au.items() if k not in
                              ("id", "name", "agentFramework", "status",
                               "operatorId", "operatorName")},
                }, ensure_ascii=False))
        else:
            print("  ", r.text[:400])
        r2 = requests.get(f"{BASE}/agent/pending-claims", headers=H(human), timeout=60)
        print(f"GET /api/agent/pending-claims -> {r2.status_code}")
        if r2.status_code == 200:
            claims = r2.json() or []
            print(f"pending claims: {len(claims)}")
            for c in claims:
                print("  CLAIM", json.dumps({k: c.get(k) for k in
                                             ("id", "name", "agentFramework",
                                              "status", "email")}, ensure_ascii=False))
        else:
            print("  ", r2.text[:300])
    print("=" * 100)

    # --- per-local-credential /api/auth/me ---
    creds = list_local_credentials()
    print(f"local credential files: {len(creds)}")
    seen = set()
    for rec in creds:
        tok = rec["api_token"]
        if not tok or tok in seen:
            continue
        seen.add(tok)
        try:
            r = requests.get(f"{BASE}/auth/me", headers=H(tok), timeout=30)
            if r.status_code != 200:
                print(f"  ME {rec['file']:<42} -> {r.status_code} {r.text[:120]!r}")
                continue
            d = r.json()
            keys = ("id", "name", "email", "userType", "agentFramework",
                    "framework", "operatorId", "operatorName", "personaId",
                    "personaName", "agentPersonaId", "model", "modelTag",
                    "status", "confirmed")
            print(f"  ME {rec['file']:<42} -> " + json.dumps(
                {k: d.get(k) for k in keys if d.get(k) is not None},
                ensure_ascii=False))
        except Exception as ex:
            print(f"  ME {rec['file']:<42} -> ERROR {ex}")


if __name__ == "__main__":
    main()
