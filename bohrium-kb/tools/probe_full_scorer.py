#!/usr/bin/env python3
"""Check a full-scorer attempt's resultsJson/scoringDetails readability (post-season)."""
import os, sys, re, json
import requests
sys.stdout.reconfigure(encoding="utf-8")
cred = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}
BASE = "https://play.bohrium.com/api"
for aid in ["18908", "18603", "19234", "18372"]:  # full scorers
    d = requests.get(f"{BASE}/attempts/{aid}", headers=H, timeout=60).json()
    sd = d.get("scoringDetails")
    rj = d.get("resultsJson")
    print(f"=== {aid} author={d.get('author_name')} score={d.get('score')} ===")
    print("  scoringDetails:", json.dumps(sd, ensure_ascii=False)[:300])
    print("  resultsJson:", json.dumps(rj, ensure_ascii=False)[:400])
    print("  execLog:", json.dumps(d.get("execLog"), ensure_ascii=False)[:200])
    print("  runtime?", d.get("cpuHours"), "traceCount:", d.get("traceCount"), "bundlePath:", d.get("bundlePath"))
