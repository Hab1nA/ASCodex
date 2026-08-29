#!/usr/bin/env python3
"""Extract full scoringDetails + resultsJson from all discoverable friday/harbor attempts."""
import json, os, re, sys, time
import requests
sys.stdout.reconfigure(encoding="utf-8")

cred = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}
BASE = "https://play.bohrium.com/api"

slugs = {
    "ppt": "construct-a-4-4-ppt-quantum-channel-and-verify-the-1c5c7f97",
    "gbsde": "reasoning-gate-gbsde-feynman-kac-e8970329",
    "permuton": "reasoning-gate-separable-covariance-1fe5635b",
    "twist": "thermodynamic-twisting-operator-diagnosis-of-perio-20fdd3fd",
    "flowforge": "flowforge-open-model-selection-flow-v5-a9464888",
    "uv": "mp-r-mp-r-a-uv-portal-a5be12b2",
    "split": "mp-r-mp-r-ab-uv-split-coann-6924985d",
    "ultrasound": "focused-imaging-and-resolution-characterisation-fr-e287fbca",
    "cnv": "multi-sample-cnv-detection-from-binned-read-counts-15924b97",
    "deepham": "solving-heterogeneous-agent-models-with-deepham-18a5adeb",
}

# known friday / our attempt ids (harvested from list + hackathon/recent)
known_ids = {
    "ppt": [], "gbsde": [], "permuton": [], "twist": [26873],
    "flowforge": [], "uv": [23701], "split": [], "ultrasound": [26978],
    "cnv": [26976], "deepham": [26975],
}

def is_friday(a):
    blob = (str(a.get("authorId","")) + str(a.get("author_name","")) + str(a.get("operatorName",""))
            + str(a.get("operatorId",""))).lower()
    return "friday" in blob or a.get("operatorId") == "1179613"

report = {}
for label, slug in slugs.items():
    report[label] = {"slug": slug, "attempts": []}
    # visible window
    r = requests.get(f"{BASE}/challenges/{slug}/attempts", params={"per_page": 1000}, headers=H, timeout=60)
    items = (r.json().get("attempts") or [])
    ids = [a["id"] for a in items if is_friday(a)]
    ids += known_ids.get(label, [])
    ids = list(dict.fromkeys(ids))
    for aid in ids:
        try:
            d = requests.get(f"{BASE}/attempts/{aid}", headers=H, timeout=60).json()
        except Exception as e:
            report[label]["attempts"].append({"id": aid, "err": str(e)})
            continue
        rec = {
            "id": aid, "author": d.get("author_name"), "operatorId": d.get("operatorId"),
            "operatorName": d.get("operatorName"), "score": d.get("score"),
            "outcome": d.get("outcome"), "createdAt": (d.get("createdAt") or "")[:16],
            "scorecard": d.get("scorecard"),
            "resultsJson": d.get("resultsJson"),
            "scoringDetails": d.get("scoringDetails"),
            "detail": d.get("detail"), "execLog": d.get("execLog"),
        }
        report[label]["attempts"].append(rec)
        sd = d.get("scoringDetails")
        print(f"=== {label} aid={aid} {d.get('author_name')} score={d.get('score')} outcome={d.get('outcome')}")
        print(f"    rj={d.get('resultsJson')}")
        print(f"    scoringDetails={json.dumps(sd, ensure_ascii=False)[:800]}")
        time.sleep(0.03)

out = os.path.join(os.path.dirname(__file__), "..", "round3_prep", "judge_feedback_detail.json")
os.makedirs(os.path.dirname(out), exist_ok=True)
json.dump(report, open(out, "w", encoding="utf-8"), ensure_ascii=False, indent=2, default=str)
print("WROTE", out)
