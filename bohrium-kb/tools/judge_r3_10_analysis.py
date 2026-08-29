#!/usr/bin/env python3
"""Round-3 Q10 (DLPFC spatial-domain) judge signal analysis.

Fetches:
1. All visible attempts for the challenge (paginated; total from response)
2. Identifies full-score (score >= 99.5) attempts
3. Pulls details for top attempts + our known attempts:
   scorecard (harbor/trace), scoringDetails, resultsJson, execLog, agent model metadata
4. Saves JSON + prints a summary.
Read-only. Does NOT submit.
"""
import json
import os
import re
import sys
import time

import requests

sys.stdout.reconfigure(encoding="utf-8")

CRED_PATH = os.path.expanduser(r"~\.dsh\bohrium_credentials.txt")
with open(CRED_PATH, encoding="utf-8") as f:
    cred = f.read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}
BASE = "https://play.bohrium.com/api"

SLUG = "spatial-domain-identification-via-graph-informed-c-35985da3-2"
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..",
                   "round3_prep", "judge_r3_10_data.json")
os.makedirs(os.path.dirname(OUT), exist_ok=True)


def fetch(url, **params):
    r = requests.get(url, headers=H, params=params or None, timeout=60)
    r.raise_for_status()
    return r.json()


def main():
    data = {"slug": SLUG, "attempts": [], "top": [], "our": []}

    # --- 1. leaderboard ---
    try:
        lb = fetch(f"{BASE}/leaderboard", challenge_id=SLUG)
        data["leaderboard"] = lb if isinstance(lb, list) else None
        print("=== LEADERBOARD (top 5) ===")
        for x in (lb[:5] if isinstance(lb, list) else []):
            print(f"  {x.get('name')} score={x.get('score')}")
    except Exception as e:
        print("leaderboard ERR", e)

    # --- 2. attempts list (paginate) ---
    seen = {}
    page = 1
    total = None
    while True:
        try:
            d = fetch(f"{BASE}/challenges/{SLUG}/attempts", per_page=50, page=page)
        except Exception as e:
            print("attempts list ERR", e)
            break
        items = d.get("attempts") or []
        if total is None:
            total = d.get("total")
        if not items:
            break
        for a in items:
            aid = a.get("id")
            if aid not in seen:
                seen[aid] = a
        page += 1
        if (fetched := len(seen)) >= (total or 0):
            break
        if page > 40:
            break
        time.sleep(0.03)

    all_attempts = list(seen.values())
    data["attempts"] = all_attempts
    scored = [a for a in all_attempts if a.get("score") is not None]
    scored.sort(key=lambda a: a.get("score") or 0, reverse=True)
    print(f"\n=== total={total} fetched={len(all_attempts)} scored={len(scored)} ===")

    # Score distribution buckets
    full = [a for a in scored if (a.get("score") or 0) >= 99.5]
    for b in [(0, 60), (60, 80), (80, 90), (90, 95), (95, 99.5), (99.5, 101)]:
        n = sum(1 for a in scored if b[0] <= (a.get("score") or 0) < b[1])
        print(f"  bucket [{b[0]},{b[1]}): {n}")
    print(f"  FULL(>=99.5): {len(full)} -> {[a.get('id') for a in full]}")

    # --- 3. top attempts detail (top 12 by score, excluding OURS) ---
    our_ids = [29104, 29105, 29106, 29108, 29124, 29127, 29136, 29139, 29143]
    top_ids = list(dict.fromkeys(
        [a.get("id") for a in full]
        + [a.get("id") for a in scored[:12] if a.get("id") not in our_ids]
    ))
    print(f"\n=== PULLING DETAILS for top attempts: {top_ids} ===")
    for aid in top_ids:
        try:
            d = fetch(f"{BASE}/attempts/{aid}")
        except Exception as e:
            print(f"  aid {aid} ERR {e}")
            continue
        rec = {
            "id": aid,
            "author_name": d.get("author_name"),
            "authorId": d.get("authorId"),
            "operatorName": d.get("operatorName"),
            "operatorId": d.get("operatorId"),
            "score": d.get("score"),
            "outcome": d.get("outcome"),
            "status": d.get("status"),
            "createdAt": (d.get("createdAt") or "")[:19],
            "scorecard": d.get("scorecard"),
            "scoringDetails": d.get("scoringDetails"),
            "resultsJson": d.get("resultsJson"),
            "answerRedacted": d.get("answerRedacted"),
            "agentName": d.get("agentName"),
            "agentFramework": d.get("agentFramework"),
            "modelTag": d.get("modelTag"),
            "execLog": d.get("execLog"),
            "detail": d.get("detail"),
            "traceHead": str(d.get("trace"))[:300],
        }
        data["top"].append(rec)
        sc = d.get("scorecard") or {}
        sd = d.get("scoringDetails") or {}
        print(f"\n--- aid={aid} name={d.get('author_name')} score={d.get('score')} "
              f"outcome={d.get('outcome')}")
        print(f"    agentFramework={d.get('agentFramework')} modelTag={d.get('modelTag')} "
              f"agentName={d.get('agentName')}")
        print(f"    scorecard={json.dumps(sc, ensure_ascii=False)[:600]}")
        print(f"    scoringDetails={json.dumps(sd, ensure_ascii=False)[:400]}")
        print(f"    resultsJson={json.dumps(d.get('resultsJson'), ensure_ascii=False)[:200]}")
        print(f"    answerRedacted={d.get('answerRedacted')}")
        time.sleep(0.03)

    # --- 4. our attempts detail ---
    print(f"\n=== OUR ATTEMPTS detail: {our_ids} ===")
    for aid in our_ids:
        try:
            d = fetch(f"{BASE}/attempts/{aid}")
        except Exception as e:
            print(f"  our aid {aid} ERR {e}")
            continue
        rec = {
            "id": aid, "author_name": d.get("author_name"), "authorId": d.get("authorId"),
            "score": d.get("score"), "outcome": d.get("outcome"), "status": d.get("status"),
            "createdAt": (d.get("createdAt") or "")[:19],
            "scorecard": d.get("scorecard"), "scoringDetails": d.get("scoringDetails"),
            "resultsJson": d.get("resultsJson"), "answerRedacted": d.get("answerRedacted"),
            "agentFramework": d.get("agentFramework"), "modelTag": d.get("modelTag"),
            "execLog": d.get("execLog"), "detail": d.get("detail"),
            "traceHead": str(d.get("trace"))[:300],
        }
        data["our"].append(rec)
        sc = d.get("scorecard") or {}
        sd = d.get("scoringDetails") or {}
        print(f"\n--- OUR aid={aid} name={d.get('author_name')} score={d.get('score')} "
              f"outcome={d.get('outcome')} status={d.get('status')}")
        print(f"    scorecard={json.dumps(sc, ensure_ascii=False)[:400]}")
        print(f"    scoringDetails={json.dumps(sd, ensure_ascii=False)[:400]}")
        print(f"    answerRedacted={d.get('answerRedacted')}")
        time.sleep(0.03)

    with open(OUT, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=2, default=str)
    print(f"\nWROTE {OUT}")


if __name__ == "__main__":
    main()
