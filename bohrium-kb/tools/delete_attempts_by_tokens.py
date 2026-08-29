#!/usr/bin/env python3
"""Delete attempts of specific identities using THEIR OWN tokens.

Reads a JSON mapping {"identity": "asp_token"} from the file given as argv[1].
For every entry:
  1. verify GET /auth/me returns exactly that identity (mismatch -> skip loudly)
  2. list attempts by author (GET /attempts?author=<id>)
  3. delete each attempt with the identity's own token (self-delete)
Guard: never delete an attempt whose score equals its challenge's current max
(recomputed live) — defense in depth; these identities are redundant anyway.

Usage: python delete_attempts_by_tokens.py <tokens.json> [--dry-run]
"""
import argparse
import json
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
BASE = "https://play.bohrium.com/api"
EPS = 1e-6
OUR_OPERATOR = "1179613"

# challenge slug -> id (for max recompute)
SLUGS = {
    "thermodynamic-twisting-operator-diagnosis-of-perio-20fdd3fd",
    "reasoning-gate-separable-covariance-1fe5635b",
    "reasoning-gate-gbsde-feynman-kac-e8970329",
    "construct-a-4-4-ppt-quantum-channel-and-verify-the-1c5c7f97",
    "flowforge-open-model-selection-flow-v5-a9464888",
    "mp-r-mp-r-a-uv-portal-a5be12b2",
    "mp-r-mp-r-ab-uv-split-coann-6924985d",
    "solving-heterogeneous-agent-models-with-deepham-18a5adeb",
    "focused-imaging-and-resolution-characterisation-fr-e287fbca",
    "multi-sample-cnv-detection-from-binned-read-counts-15924b97",
}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("tokens_file")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    mapping = json.load(open(args.tokens_file, encoding="utf-8"))
    print(f"[info] {len(mapping)} identities from {args.tokens_file}")

    # 1. verify each token -> identity
    verified = {}
    for ident, tok in mapping.items():
        r = requests.get(f"{BASE}/auth/me", headers={"Authorization": f"Bearer {tok}"}, timeout=60)
        d = r.json() if r.status_code == 200 else {}
        got = d.get("id")
        if got != ident:
            print(f"[FAIL] token for {ident} resolved to {got} (HTTP {r.status_code}) — SKIP")
            continue
        # also sanity: agent account bound to our operator
        op = d.get("operatorId")
        if op not in (OUR_OPERATOR, None):
            print(f"[warn] {ident} operatorId={op} (not {OUR_OPERATOR}) — still deleting its own attempts")
        verified[ident] = tok
    print(f"[info] verified {len(verified)}/{len(mapping)} tokens")

    # 2. live challenge maxes (per challengeId)
    ch_max = {}
    for slug in SLUGS:
        page, fetched, total = 1, 0, 0
        while True:
            r = requests.get(f"{BASE}/challenges/{slug}/attempts",
                             params={"per_page": 20, "page": page}, timeout=90)
            d = r.json() or {}
            items = d.get("attempts") or []
            total = d.get("total", 0)
            fetched += len(items)
            for a in items:
                if str(a.get("operatorId") or "") == OUR_OPERATOR:
                    sc = a.get("score") or 0.0
                    ch_max[slug] = max(ch_max.get(slug, 0.0), sc)
            if fetched >= total or not items:
                break
            page += 1

    # 3. list + delete
    n_del = n_skip = n_fail = 0
    for ident, tok in verified.items():
        H = {"Authorization": f"Bearer {tok}"}
        r = requests.get(f"{BASE}/attempts", headers=H,
                         params={"author": ident, "limit": 100}, timeout=60)
        try:
            items = r.json().get("attempts") or []
        except ValueError:
            print(f"[warn] {ident}: cannot parse attempt list: {r.text[:150]}")
            continue
        print(f"\n{ident}: {len(items)} attempts")
        for a in items:
            aid = a.get("id")
            cid = a.get("challengeId") or ""
            sc = a.get("score") or 0.0
            if abs(sc - ch_max.get(cid, -1.0)) < EPS:
                print(f"  [GUARD] aid={aid} ({cid}, score={sc}) is at challenge max — SKIP")
                n_skip += 1
                continue
            print(f"  DEL aid={aid} ({cid[:20]}, score={sc})")
            if args.dry_run:
                continue
            rd = requests.delete(f"{BASE}/attempts/{aid}", headers=H, timeout=60)
            if rd.status_code < 300:
                n_del += 1
            else:
                n_fail += 1
                print(f"  FAIL aid={aid} -> {rd.status_code} {rd.text[:120]}")
    print(f"\n[result] deleted={n_del} guarded={n_skip} failed={n_fail}")


if __name__ == "__main__":
    main()
