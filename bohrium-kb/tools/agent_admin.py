#!/usr/bin/env python3
"""Agent-account admin operations that require the HUMAN token
(Profile -> API Tokens -> create; starts with asp_).

Subcommands:
  list            GET /agent/register + /agent/pending-claims (authoritative roster)
  reject          POST /agent/reject/{id}  (unbind from your account; dry-run default)
  delete-attempts DELETE /attempts/{id}    for given identities (dry-run default)

Examples:
  python agent_admin.py list --human-token asp_xxx
  python agent_admin.py reject --human-token asp_xxx --ids friday-u3 friday-u4-52367
  python agent_admin.py reject --human-token asp_xxx --all-redundant
  python agent_admin.py delete-attempts --human-token asp_xxx --all-redundant --execute
"""
import argparse
import json
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
BASE = "https://play.bohrium.com/api"
EPS = 1e-6

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


def H(tok):
    return {"Authorization": f"Bearer {tok}"}


def get(path, tok, **kw):
    r = requests.get(f"{BASE}{path}", headers=H(tok), timeout=60, **kw)
    return r


def redundant_identities(tok):
    """Recompute keepers/redundant from live attempts (same rule as cleanup script)."""
    headers = H(tok)
    team = []
    for slug in SLUGS.values():
        page, fetched, total = 1, 0, 0
        while True:
            r = get(f"/challenges/{slug}/attempts", tok, params={"per_page": 20, "page": page})
            d = r.json() or {}
            items = d.get("attempts") or []
            total = d.get("total", 0)
            fetched += len(items)
            team.extend(items)
            if fetched >= total or not items:
                break
            page += 1
            if page > 200:
                break
    ch_max = {}
    for a in team:
        if str(a.get("operatorId") or "") == OUR_OPERATOR:
            sc = a.get("score") or 0.0
            cid = a.get("challengeId") or ""
            ch_max[cid] = max(ch_max.get(cid, 0.0), sc)
    keepers = set()
    for a in team:
        if str(a.get("operatorId") or "") == OUR_OPERATOR and \
           abs((a.get("score") or 0.0) - ch_max.get(a.get("challengeId") or "", 0.0)) < EPS:
            keepers.add(str(a.get("authorId") or ""))
    redundant = sorted({str(a.get("authorId") or "") for a in team
                        if str(a.get("operatorId") or "") == OUR_OPERATOR} - keepers)
    return keepers, redundant


def cmd_list(args):
    r = get("/agent/register", args.human_token)
    print("GET /agent/register ->", r.status_code)
    if r.status_code == 200:
        regs = r.json()
        print(f"registered agent accounts: {len(regs)}")
        for e in regs:
            au = e.get("agentUser") or {}
            print(f"  {au.get('id'):<24} {au.get('name',''):<24} "
                  f"framework={au.get('agentFramework','')}")
    else:
        print(r.text[:300])
    r2 = get("/agent/pending-claims", args.human_token)
    print("GET /agent/pending-claims ->", r2.status_code)
    if r2.status_code == 200:
        claims = r2.json()
        print(f"pending claims: {len(claims)}")
        for c in claims:
            print(f"  {c.get('id'):<24} {c.get('name','')}")


def cmd_reject(args):
    keepers, redundant = redundant_identities(args.human_token)
    print(f"[info] keepers={len(keepers)} redundant={len(redundant)}")
    ids = args.ids or (redundant if args.all_redundant else [])
    if not ids:
        print("no ids given; use --ids or --all-redundant")
        return
    for i in ids:
        if i in keepers:
            print(f"  [warn] {i} is a KEEPER (holds a challenge max) — refusing")
            continue
        if not args.execute:
            print(f"  dry-run POST /agent/reject/{i}")
            continue
        r = requests.post(f"{BASE}/agent/reject/{i}", headers=H(args.human_token), timeout=60)
        print(f"  reject {i} -> {r.status_code} {r.text[:150]}")


def cmd_delete_attempts(args):
    keepers, redundant = redundant_identities(args.human_token)
    print(f"[info] keepers={len(keepers)} redundant={len(redundant)}")
    ids = args.ids or (redundant if args.all_redundant else [])
    if not ids:
        print("no ids given; use --ids or --all-redundant")
        return
    to_del = []
    for slug in SLUGS.values():
        page, fetched, total = 1, 0, 0
        while True:
            r = get(f"/challenges/{slug}/attempts", args.human_token,
                    params={"per_page": 20, "page": page})
            d = r.json() or {}
            items = d.get("attempts") or []
            total = d.get("total", 0)
            fetched += len(items)
            for a in items:
                au = str(a.get("authorId") or "")
                if au in ids and au not in keepers:
                    to_del.append((au, a.get("id"), a.get("score")))
            if fetched >= total or not items:
                break
            page += 1
            if page > 200:
                break
    print(f"[plan] {len(to_del)} attempts to delete")
    if not args.execute:
        for au, aid, sc in sorted(to_del):
            print(f"  dry-run DEL aid={aid} ({au}, score={sc})")
        return
    n_ok = n_fail = 0
    for au, aid, sc in sorted(to_del):
        r = requests.delete(f"{BASE}/attempts/{aid}", headers=H(args.human_token), timeout=60)
        if r.status_code < 300:
            n_ok += 1
        else:
            n_fail += 1
            print(f"  FAIL aid={aid} ({au}) -> {r.status_code} {r.text[:120]}")
    print(f"[execute] deleted {n_ok}, failed {n_fail}")


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name, fn in [("list", cmd_list), ("reject", cmd_reject),
                     ("delete-attempts", cmd_delete_attempts)]:
        sp = sub.add_parser(name)
        sp.add_argument("--human-token", required=True)
        sp.add_argument("--ids", nargs="*", default=None)
        sp.add_argument("--all-redundant", action="store_true")
        sp.add_argument("--execute", action="store_true")
    args = ap.parse_args()
    {"list": cmd_list, "reject": cmd_reject, "delete-attempts": cmd_delete_attempts}[args.cmd](args)


if __name__ == "__main__":
    main()
