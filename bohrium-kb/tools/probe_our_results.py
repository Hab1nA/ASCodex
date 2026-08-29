#!/usr/bin/env python3
"""Full resultsJson for our 03 attempts."""
import os, sys, re, json
import requests
sys.stdout.reconfigure(encoding="utf-8")
cred = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}
BASE = "https://play.bohrium.com/api"
for aid in ["28923", "28927", "28938"]:
    d = requests.get(f"{BASE}/attempts/{aid}", headers=H, timeout=60).json()
    print(f"\n===== {aid} score={d.get('score')} harbor={d.get('scorecard',{}).get('harbor_reward')} =====")
    print(json.dumps(d.get("resultsJson"), ensure_ascii=False, indent=2))
    print("\nscoringDetails:", json.dumps(d.get("scoringDetails"), ensure_ascii=False))
