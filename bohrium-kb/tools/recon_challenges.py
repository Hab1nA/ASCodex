#!/usr/bin/env python3
"""Round-4 recon: verify Friday identity, list current challenges with popularity stats.

Usage: python recon_challenges.py [cred_file]
Output: JSON summary on stdout + full dump to _logs/recon_<ts>.json
"""
import json
import os
import re
import sys
import time
from pathlib import Path

import requests

BASE = "https://play.bohrium.com/api"


def load_token(cred_path: str) -> str:
    txt = Path(cred_path).read_text(encoding="utf-8")
    m = re.search(r"api_token\s*=\s*(\S+)", txt)
    if not m:
        sys.exit("no api_token in " + cred_path)
    return m.group(1)


def main():
    cred = sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser("~/.dsh/bohrium_credentials.txt")
    tok = load_token(cred)
    h = {"Authorization": f"Bearer {tok}"}
    ts = time.strftime("%Y%m%d_%H%M%S")
    out = {"ts": ts, "identity": {}, "hackathon": None, "challenges": None, "errors": []}

    r = requests.get(BASE + "/auth/me", headers=h, timeout=60)
    out["identity"] = {"status": r.status_code, "body": r.json() if r.headers.get("content-type", "").startswith("application/json") else r.text[:300]}

    try:
        r = requests.get(BASE + "/hackathon/current", headers=h, timeout=60)
        out["hackathon"] = {"status": r.status_code, "body": r.json()}
    except Exception as e:
        out["errors"].append(f"hackathon: {e}")

    for qs in ["", "?origin=hackathon"]:
        r = requests.get(BASE + "/challenges" + qs, headers=h, timeout=60)
        key = "challenges" if qs == "" else "challenges_hackathon"
        out[key] = {"status": r.status_code, "body": r.json() if r.headers.get("content-type", "").startswith("application/json") else r.text[:500]}
        if r.status_code == 200:
            break

    logdir = Path(r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\_logs")
    logdir.mkdir(exist_ok=True)
    dump = logdir / f"recon_{ts}.json"
    dump.write_text(json.dumps(out, ensure_ascii=False, indent=1), encoding="utf-8")

    # compact print
    me = out["identity"]["body"]
    print("IDENTITY:", json.dumps(me, ensure_ascii=False)[:600])
    if out.get("hackathon") and isinstance(out["hackathon"].get("body"), dict):
        hb = out["hackathon"]["body"]
        print("HACKATHON keys:", list(hb.keys()))
        print("HACKATHON:", json.dumps(hb, ensure_ascii=False)[:1200])
    ch = out.get("challenges") or out.get("challenges_hackathon")
    if ch and ch["status"] == 200 and isinstance(ch["body"], list):
        print(f"CHALLENGES: {len(ch['body'])}")
        for c in ch["body"]:
            keys = {k: c.get(k) for k in ("id", "slug", "title", "disc", "discipline", "difficulty", "status", "origin", "attempt_count", "attempts", "participants", "solved", "best_score", "max_score", "points", "score", "created_at") if k in c}
            print(json.dumps(keys, ensure_ascii=False)[:400])
    else:
        print("CHALLENGES raw:", json.dumps(ch, ensure_ascii=False)[:2000] if ch else "none")
    print("DUMP:", dump)


if __name__ == "__main__":
    main()

