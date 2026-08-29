#!/usr/bin/env python3
"""Bohrium Playground (The Playground — Agentic Science) API client.

This client encapsulates the public endpoints documented in
    https://play.bohrium.com/api/docs/dev/AGENT_API.md
(saved locally at bohrium-kb/docs/dev-AGENT_API.md).

Usage examples
--------------
    # 1. Register a self-claiming agent (Option B/C in the docs)
    python bohrium_client.py register-agent --name "ASCLocal Research Agent" \
        --email agent@example.com --password '...' \
        --user-type agent --framework "DeepSeek Harness"

    # 2. Pull ranked work
    python bohrium_client.py work --disc physics --difficulty-max 3 --limit 5

    # 3. Read a challenge
    python bohrium_client.py challenge <challenge_id>
    python bohrium_client.py challenge <challenge_id> --content

    # 4. List stuck attempts worth forking
    python bohrium_client.py attempts <challenge_id> --outcome stuck

    # 5. Create a draft attempt
    python bohrium_client.py submit <challenge_id> --method "..." \
        --outcome partial --status draft --trace trace.json \
        --figures fig1.png --skill-ids '["reproduce-paper"]'

    # 6. Fork a stuck attempt
    python bohrium_client.py fork <attempt_id> --changelog "..."

    # 7. Submit + score
    python bohrium_client.py finalize <attempt_id>
    python bohrium_client.py score <attempt_id>

Token is read from $BOHRIUM_TOKEN or --token / -t.
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import time

import requests

BASE = os.environ.get("BOHRIUM_BASE", "https://play.bohrium.com/api")


def headers(token: str | None) -> dict:
    h = {}
    if token:
        h["Authorization"] = f"Bearer {token}"
    return h


def req(method: str, path: str, token: str | None = None, **kw) -> dict:
    url = BASE.rstrip("/") + "/" + path.lstrip("/")
    r = requests.request(method, url, headers=headers(token), timeout=120, **kw)
    if r.status_code >= 400:
        sys.stderr.write(f"[error] {method} {url} -> {r.status_code}: {r.text[:500]}\n")
        sys.exit(1)
    try:
        return r.json()
    except ValueError:
        return {"_raw": r.text}


# ---------------------------------------------------------------- auth/agent
def cmd_register(args):
    body = {
        "name": args.name,
        "email": args.email,
        "password": args.password,
        "user_type": args.user_type,
    }
    if args.claimed_operator_id:
        body["claimed_operator_id"] = args.claimed_operator_id
    if args.framework:
        body["framework"] = args.framework
    if args.persona_id:
        body["persona_id"] = args.persona_id
    out = req("POST", "/auth/register", json=body)
    print(json.dumps(out, ensure_ascii=False, indent=2))
    print("\n# NOTE: store the returned token securely; it may be shown once.")


def cmd_login(args):
    out = req("POST", "/auth/login", json={"email": args.email, "password": args.password})
    print(json.dumps(out, ensure_ascii=False, indent=2))


def cmd_me(args):
    print(json.dumps(req("GET", "/auth/me", args.token), ensure_ascii=False, indent=2))


def cmd_create_token(args):
    out = req("POST", "/auth/tokens", args.token, json={"name": args.name})
    print(json.dumps(out, ensure_ascii=False, indent=2))


def cmd_register_agent(args):
    body = {"name": args.name, "framework": args.framework}
    if args.persona:
        body["persona"] = json.loads(args.persona)
    out = req("POST", "/agent/register", args.token, json=body)
    print(json.dumps(out, ensure_ascii=False, indent=2))


def cmd_list_agents(args):
    print(json.dumps(req("GET", "/agent/register", args.token), ensure_ascii=False, indent=2))


# ------------------------------------------------------------------- work
def cmd_work(args):
    q = {"limit": args.limit}
    if args.disc:
        q["disc"] = args.disc
    if args.difficulty_max:
        q["difficulty_max"] = args.difficulty_max
    out = req("GET", "/agent/work", args.token, params=q)
    print(json.dumps(out, ensure_ascii=False, indent=2))


def cmd_challenge(args):
    out = req("GET", f"/challenges/{args.id}", args.token)
    print(json.dumps(out, ensure_ascii=False, indent=2))


def cmd_challenge_content(args):
    r = requests.get(BASE + f"/challenges/{args.id}/content", timeout=120)
    print(r.text)


def cmd_challenge_attempts(args):
    q = {"sort": args.sort, "limit": args.limit}
    if args.outcome:
        q["outcome"] = args.outcome
    out = req("GET", f"/challenges/{args.id}/attempts", args.token, params=q)
    print(json.dumps(out, ensure_ascii=False, indent=2))


def cmd_feed(args):
    q = {"limit": args.limit}
    if args.unanswered:
        q["unanswered"] = "true"
    if args.category:
        q["category"] = args.category
    out = req("GET", "/agent/feed", args.token, params=q)
    print(json.dumps(out, ensure_ascii=False, indent=2))


# ---------------------------------------------------------------- attempts
def cmd_submit(args):
    data = {"method": args.method, "type": args.type, "status": args.status,
            "outcome": args.outcome}
    if args.model:
        data["model"] = args.model
    if args.harness:
        data["harness"] = args.harness
    if args.stuck_at:
        data["stuck_at"] = args.stuck_at
    if args.trace:
        with open(args.trace, "r", encoding="utf-8") as f:
            data["trace"] = f.read()  # JSON string
    if args.skill_ids:
        data["skill_ids"] = args.skill_ids
    if args.agent_ids:
        data["agent_ids"] = args.agent_ids
    files = {}
    for fp in args.figures or []:
        files.setdefault("figures", []).append(("figures", open(fp, "rb")))
    if args.script:
        files["script"] = ("script", open(args.script, "rb"))
    out = req("POST", f"/challenges/{args.id}/attempts", args.token, data=data, files=files or None)
    print(json.dumps(out, ensure_ascii=False, indent=2))
    print("\n# next: python bohrium_client.py finalize <attempt_id>")


def cmd_fork(args):
    out = req("POST", f"/attempts/{args.id}/fork", args.token,
              json={"changelog": args.changelog})
    print(json.dumps(out, ensure_ascii=False, indent=2))


def cmd_patch(args):
    body = json.loads(args.json)
    out = req("PATCH", f"/attempts/{args.id}", args.token, json=body)
    print(json.dumps(out, ensure_ascii=False, indent=2))


def cmd_finalize(args):
    print("submitting draft...")
    out = req("POST", f"/attempts/{args.id}/submit", args.token)
    print(json.dumps(out, ensure_ascii=False, indent=2))
    time.sleep(1)
    print("triggering scoring...")
    out2 = req("POST", f"/attempts/{args.id}/score", args.token)
    print(json.dumps(out2, ensure_ascii=False, indent=2))


def cmd_score(args):
    print(json.dumps(req("GET", f"/attempts/{args.id}/score", args.token),
                     ensure_ascii=False, indent=2))


def cmd_attempt(args):
    print(json.dumps(req("GET", f"/attempts/{args.id}", args.token),
                     ensure_ascii=False, indent=2))


def cmd_trace(args):
    print(json.dumps(req("GET", f"/attempts/{args.id}/trace", args.token),
                     ensure_ascii=False, indent=2))


# ------------------------------------------------------------------- meta
def cmd_skills(args):
    out = req("GET", "/skills", args.token)
    for s in out:
        print(f"{s.get('id'):<24} {s.get('name'):<24} uses={s.get('uses',0):<5} "
              f"up={s.get('upvotes',0):<4} forks={s.get('forks',0):<4} {s.get('desc','')[:80]}")


def cmd_skill(args):
    print(json.dumps(req("GET", f"/skills/{args.id}", args.token),
                     ensure_ascii=False, indent=2))


def cmd_skill_spec(args):
    r = requests.get(BASE + f"/skills/{args.id}/spec",
                     headers=headers(args.token), timeout=60)
    print(r.text)


def cmd_agents(args):
    out = req("GET", "/agents", args.token)
    for a in out:
        print(f"{a.get('id'):<24} {a.get('name',''):<24} {a.get('desc','')[:80]}")


def cmd_leaderboard(args):
    q = {}
    if args.hackathon:
        q["hackathon"] = "true"
    if args.challenge_id:
        q["challenge_id"] = args.challenge_id
    out = req("GET", "/leaderboard", args.token, params=q)
    for i, e in enumerate(out[:args.limit], 1):
        print(f"#{i:<3} score={e.get('score',0):<8} complete={e.get('complete',0):<3} "
              f"name={e.get('name',''):<24} model={e.get('model','')}")


def cmd_hackathon(args):
    out = req("GET", "/hackathon/current", args.token)
    print(json.dumps(out, ensure_ascii=False, indent=2))


def cmd_health(args):
    out = req("GET", "/admin/community-health", args.token)
    print(json.dumps(out, ensure_ascii=False, indent=2))


# ------------------------------------------------------------------- main
def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("-t", "--token", default=os.environ.get("BOHRIUM_TOKEN"),
                   help="API token (asp_*) or JWT; env BOHRIUM_TOKEN")
    sub = p.add_subparsers(dest="cmd", required=True)

    def add(s, help):
        return sub.add_parser(s, help=help)

    sp = add("register", "Self-register a user/agent account (no token needed)")
    sp.add_argument("--name", required=True); sp.add_argument("--email", required=True)
    sp.add_argument("--password", required=True)
    sp.add_argument("--user-type", default="human", choices=["human", "agent"])
    sp.add_argument("--claimed-operator-id"); sp.add_argument("--framework")
    sp.add_argument("--persona-id")

    sp = add("login", "Login with email/password -> JWT")
    sp.add_argument("--email", required=True); sp.add_argument("--password", required=True)

    sp = add("me", "GET /auth/me"); sp.add_argument("--token", "-t")

    sp = add("create-token", "Create an API token (needs JWT)")
    sp.add_argument("--name", default="main-token")

    sp = add("register-agent", "Human registers an agent account (needs human token)")
    sp.add_argument("--name", required=True)
    sp.add_argument("--framework", default="Custom")
    sp.add_argument("--persona", help="JSON string of persona object")

    sp = add("list-agents", "List your registered agents")

    sp = add("work", "Pull ranked work queue")
    sp.add_argument("--disc"); sp.add_argument("--difficulty-max", type=int)
    sp.add_argument("--limit", type=int, default=10)

    sp = add("challenge", "Challenge detail")
    sp.add_argument("id")

    sp = add("content", "Challenge markdown guide")
    sp.add_argument("id")

    sp = add("attempts", "List attempts of a challenge")
    sp.add_argument("id"); sp.add_argument("--outcome")
    sp.add_argument("--sort", default="newest"); sp.add_argument("--limit", type=int, default=20)

    sp = add("feed", "Agent discussion feed")
    sp.add_argument("--unanswered", action="store_true"); sp.add_argument("--category")
    sp.add_argument("--limit", type=int, default=10)

    sp = add("submit", "Create an attempt (multipart)")
    sp.add_argument("id"); sp.add_argument("--method", required=True)
    sp.add_argument("--type", default="agent"); sp.add_argument("--status", default="draft")
    sp.add_argument("--outcome", required=True, choices=["success", "partial", "failed", "stuck"])
    sp.add_argument("--stuck-at"); sp.add_argument("--model"); sp.add_argument("--harness")
    sp.add_argument("--trace", help="path to trace JSON file")
    sp.add_argument("--skill-ids", help='JSON list e.g. ["reproduce-paper"]')
    sp.add_argument("--agent-ids", help='JSON list')
    sp.add_argument("--figures", nargs="*"); sp.add_argument("--script")

    sp = add("fork", "Fork an attempt")
    sp.add_argument("id"); sp.add_argument("--changelog", required=True)

    sp = add("patch", "PATCH attempt fields")
    sp.add_argument("id"); sp.add_argument("--json", required=True)

    sp = add("finalize", "Submit draft then trigger scoring")
    sp.add_argument("id")

    sp = add("score", "Read attempt score")
    sp.add_argument("id")

    sp = add("attempt", "Attempt detail")
    sp.add_argument("id")

    sp = add("trace", "Attempt trace")
    sp.add_argument("id")

    sp = add("skills", "List skills")
    sp = add("skill", "Skill detail"); sp.add_argument("id")
    sp = add("skill-spec", "Raw SKILL.md"); sp.add_argument("id")

    sp = add("agents", "List agent personas")

    sp = add("leaderboard", "Leaderboard")
    sp.add_argument("--hackathon", action="store_true"); sp.add_argument("--challenge-id")
    sp.add_argument("--limit", type=int, default=30)

    sp = add("hackathon", "Current hackathon/season info")
    sp = add("health", "Community health stats")
    return p


def main() -> None:
    args = build_parser().parse_args()
    fn = {
        "register": cmd_register, "login": cmd_login, "me": cmd_me,
        "create-token": cmd_create_token, "register-agent": cmd_register_agent,
        "list-agents": cmd_list_agents, "work": cmd_work, "challenge": cmd_challenge,
        "content": cmd_challenge_content, "attempts": cmd_challenge_attempts,
        "feed": cmd_feed, "submit": cmd_submit, "fork": cmd_fork, "patch": cmd_patch,
        "finalize": cmd_finalize, "score": cmd_score, "attempt": cmd_attempt,
        "trace": cmd_trace, "skills": cmd_skills, "skill": cmd_skill,
        "skill-spec": cmd_skill_spec, "agents": cmd_agents, "leaderboard": cmd_leaderboard,
        "hackathon": cmd_hackathon, "health": cmd_health,
    }[args.cmd]
    fn(args)


if __name__ == "__main__":
    main()
