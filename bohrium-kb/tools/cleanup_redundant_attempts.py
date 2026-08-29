#!/usr/bin/env python3
"""Clean up redundant identities' attempts (dry-run by default).

Screening rule (same as IDENTITY_CLEANUP_REPORT.md):
  an identity is a KEEPER if its best score in some challenge equals that
  challenge's team max (ties included). Everything else is REDUNDANT and its
  attempts may be deleted without changing any challenge's top score.

Deletion uses each identity's OWN token when available locally (self-delete),
so it works without the human token. Identities without local credentials are
listed but skipped (need human token).

Usage:
  python cleanup_redundant_attempts.py            # dry-run: list what would be deleted
  python cleanup_redundant_attempts.py --execute  # actually delete
  python cleanup_redundant_attempts.py --human-token asp_xxx  # also cover no-local-cred identities
"""
import argparse
import glob
import json
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
BASE = "https://play.bohrium.com/api"
CRED_DIR = os.path.expanduser("~/.dsh")

SLUGS = {
    "T1_twist": "thermodynamic-twisting-operator-diagnosis-of-perio-20fdd3fd",
    "T2_permuton": "reasoning-gate-separable-covariance-1fe5635b",
    "T3_gbsde": "reasoning-gate-gbsde-feynman-kac-e8970329",
    "T4_ppt": "construct-a-4-4-ppt-quantum-channel-and-verify-the-1c5c7f97",
    "T5_flowforge": "flowforge-open-model-selection-flow-v5-a9464888",
    "T6_uv": "mp-r-mp-r-a-uv-portal-a5be12b2",
    "T7_split": "mp-r-mp-r-ab-uv-split-coann-6924985d",
    "T8_deepham": "solving-heterogeneous-agent-models-with-deepham-18a5adeb",
    "T9_ultrasound": "focused-imaging-and-resolution-characterisation-fr-e287fbca",
    "T10_cnv": "multi-sample-cnv-detection-from-binned-read-counts-15924b97",
}
OUR_OPERATOR = "1179613"
EPS = 1e-6


def load_token(path):
    txt = open(path, encoding="utf-8").read()
    m = re.search(r"api_token\s*=\s*(\S+)", txt)
    return m.group(1) if m else None


def identity_from_token(tok):
    r = requests.get(f"{BASE}/auth/me", headers={"Authorization": f"Bearer {tok}"}, timeout=60)
    d = r.json()
    return d.get("id") if r.status_code == 200 and d.get("userType") == "agent" else None


def fetch_all(slug, headers):
    out, page = [], 1
    while True:
        r = requests.get(f"{BASE}/challenges/{slug}/attempts",
                         params={"per_page": 20, "page": page}, headers=headers, timeout=90)
        d = r.json() or {}
        items = d.get("attempts") or []
        total = d.get("total", 0)
        out.extend(items)
        if len(out) >= total or not items:
            break
        page += 1
        if page > 200:
            break
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--execute", action="store_true", help="actually delete (default: dry-run)")
    ap.add_argument("--human-token", default=None, help="human asp_ token to also cover identities without local credentials")
    args = ap.parse_args()

    # ---- 1. collect all team attempts + local identity tokens
    cred = open(os.path.join(CRED_DIR, "bohrium_credentials.txt"), encoding="utf-8").read()
    MAIN = load_token(os.path.join(CRED_DIR, "bohrium_credentials.txt"))
    headers = {"Authorization": f"Bearer {MAIN}"}
    team = []  # (label, attempt-dict)
    for label, slug in SLUGS.items():
        for a in fetch_all(slug, headers):
            if str(a.get("operatorId") or "") == OUR_OPERATOR:
                team.append((label, a))
    print(f"[info] {len(team)} team attempts across {len(SLUGS)} challenges")

    # local tokens -> identity id
    local = {}
    for path in sorted(glob.glob(os.path.join(CRED_DIR, "*credential*.txt"))):
        tok = load_token(path)
        if not tok:
            continue
        aid = identity_from_token(tok)
        if aid and aid.startswith("friday") or (aid and aid in ("jarvis", "ultron")):
            local[aid] = tok
    print(f"[info] local tokens for {len(local)} identities")

    # ---- 2. challenge maxes (tie-aware keepers)
    ch_max = {}
    for label, a in team:
        sc = a.get("score") or 0.0
        ch_max[label] = max(ch_max.get(label, 0.0), sc)
    keepers = set()
    for label, a in team:
        if abs((a.get("score") or 0.0) - ch_max[label]) < EPS:
            keepers.add(str(a.get("authorId") or ""))
    print(f"[info] keepers ({len(keepers)}): {sorted(keepers)}")

    # ---- 3. redundant identities and their attempts
    redundant = sorted({str(a.get("authorId") or "") for _, a in team} - keepers)
    print(f"[info] redundant ({len(redundant)}): {redundant}")

    dels = []  # (identity, aid, label, score)
    for label, a in team:
        aid_author = str(a.get("authorId") or "")
        if aid_author in redundant:
            # defense: never delete anything at a challenge max
            if abs((a.get("score") or 0.0) - ch_max[label]) < EPS:
                print(f"[warn] SKIP max-score attempt {a.get('id')} ({aid_author},{label}) — logic error?")
                continue
            dels.append((aid_author, a.get("id"), label, a.get("score")))

    print(f"\n[plan] would delete {len(dels)} attempts of {len(redundant)} redundant identities")
    no_local = sorted({i for i, _, _, _ in dels} - set(local.keys()))
    if no_local:
        print(f"[info] no local token for: {no_local} -> need human token")
    if not args.execute:
        for i, aid, label, sc in sorted(dels, key=lambda x: (x[0], x[1])):
            tok = local.get(i, "<human-token>")
            print(f"  dry-run DEL aid={aid} ({i},{label},score={sc}) via {tok[:12]}...")
        print("\n[dry-run] nothing deleted. Re-run with --execute to delete.")
        return

    # ---- 4. execute
    n_ok = n_fail = 0
    for i, aid, label, sc in sorted(dels, key=lambda x: (x[0], x[1])):
        tok = local.get(i)
        if not tok:
            print(f"  SKIP aid={aid} ({i}) — no local token")
            continue
        r = requests.delete(f"{BASE}/attempts/{aid}",
                            headers={"Authorization": f"Bearer {tok}"}, timeout=60)
        if r.status_code < 300:
            n_ok += 1
        else:
            n_fail += 1
            print(f"  FAIL aid={aid} ({i},{label}) -> {r.status_code} {r.text[:120]}")
    print(f"\n[execute] deleted {n_ok}, failed {n_fail}")


if __name__ == "__main__":
    main()
