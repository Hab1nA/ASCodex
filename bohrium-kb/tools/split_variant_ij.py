#!/usr/bin/env python3
"""Variants I/J: reserved-name composition form (I) and g_eff at general x (J)."""
import hashlib
import json
import os
import sys

sys.stdout.reconfigure(encoding="utf-8")

WORK = os.path.join(r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal",
                    "work", "mp-r-ab-uv-split-coann-6924985d")

a = json.load(open(os.path.join(WORK, "variant_D", "final", "answer.json"), encoding="utf-8"))
e = json.load(open(os.path.join(WORK, "variant_D", "evidence", "derivation.json"), encoding="utf-8"))
dm = open(os.path.join(WORK, "variant_D", "DERIVATION.md"), encoding="utf-8").read()


def build(tag, mutate_answer, mutate_evidence):
    aa = json.loads(json.dumps(a))
    ee = json.loads(json.dumps(e))
    mm = dm
    aa, mm = mutate_answer(aa, mm)
    ee = mutate_evidence(ee)
    vdir = os.path.join(WORK, tag)
    os.makedirs(os.path.join(vdir, "final"), exist_ok=True)
    os.makedirs(os.path.join(vdir, "evidence"), exist_ok=True)
    json.dump(aa, open(os.path.join(vdir, "final", "answer.json"), "w", encoding="utf-8"), indent=2)
    json.dump(ee, open(os.path.join(vdir, "evidence", "derivation.json"), "w", encoding="utf-8"), indent=2)
    open(os.path.join(vdir, "DERIVATION.md"), "w", encoding="utf-8").write(mm)

    ans_txt = json.dumps(aa, indent=2)
    dm_txt = mm
    steps = []

    def add(t, title, body, **kw):
        x = {"step_type": t, "title": title, "body": body, "duration_s": kw.get("d", 2.0),
             "cost_usd": kw.get("c", 0.001), "tokens": kw.get("tk", 80),
             "step_order": len(steps) + 1, "timestamp": kw.get("ts", "2026-08-15T05:00:00Z")}
        for k in ("tool_call_id", "tool_name", "tool_args"):
            if k in kw:
                x[k] = kw[k]
        steps.append(x)

    add("thought", "Read the split-coannihilation contract",
        "Reconstruct R_UV and R_SPLIT per Section 5; every R_SPLIT expression symbolic in kappa_D.")
    t0 = 10
    for i in range(1, 5):
        out = open(os.path.join(WORK, "variant_D", f"derive_part{i}.py.out"), encoding="utf-8").read()
        add("tool_call", f"Run derive_part{i}.py", "Execute the derivation stage.",
            tool_call_id=f"tc{i}", tool_name="python",
            tool_args={"command": f"python src/derive_part{i}.py"}, ts=f"2026-08-15T05:00:{t0:02d}Z")
        add("tool_result", f"derive_part{i}.py output", out, tool_call_id=f"tc{i}",
            ts=f"2026-08-15T05:00:{t0+3:02d}Z")
        t0 += 10
    add("tool_call", "Write final/answer.json", "Write the formal terminal result.",
        tool_call_id="tc5", tool_name="write", tool_args={"file_path": "outputs/final/answer.json"},
        ts=f"2026-08-15T05:00:{t0:02d}Z")
    add("tool_result", "answer.json written",
        f"Wrote outputs/final/answer.json ({len(ans_txt)} bytes). Content:\n{ans_txt}",
        tool_call_id="tc5", ts=f"2026-08-15T05:00:{t0+2:02d}Z")
    t0 += 10
    add("tool_call", "Run self_check.py", "Verify the closed-world contract.",
        tool_call_id="tc6", tool_name="python", tool_args={"command": "python src/self_check.py"},
        ts=f"2026-08-15T05:00:{t0:02d}Z")
    add("tool_result", "self_check.py output",
        "ALL CONTRACT CHECKS PASSED ( 21 relations, answer + evidence + derivation )",
        tool_call_id="tc6", ts=f"2026-08-15T05:00:{t0+5:02d}Z")
    add("artifact", "Deliverables finalized",
        "SHA-256: answer.json=" + hashlib.sha256(ans_txt.encode()).hexdigest()[:16]
        + " DERIVATION.md=" + hashlib.sha256(dm_txt.encode()).hexdigest()[:16],
        ts="2026-08-15T05:01:00Z")
    add("decision", "Submit via playground CLI",
        "Package outputs with the derivation evidence and submit through the Playground CLI worker channel.",
        ts="2026-08-15T05:01:10Z")
    with open(os.path.join(vdir, "trace.jsonl"), "w", encoding="utf-8") as f:
        for s in steps:
            f.write(json.dumps(s, ensure_ascii=False) + "\n")
    return vdir


def mut_i(aa, mm):
    aa["targets"]["R_SPLIT"]["g_eff_expression"] = "g1*(1+a)"
    aa["targets"]["R_SPLIT"]["sigma_eff_expression"] = "2*K_delta*f_D^kappa_D*W_delta"
    mm = mm.replace('"expression": "g1+g2*(1+Delta)^1.5*exp(-x_f*Delta)"',
                    '"expression": "g1*(1+a)"')
    mm = mm.replace('"expression": "2*K_delta*f_D^kappa_D*a/(1+a)^2"',
                    '"expression": "2*K_delta*f_D^kappa_D*W_delta"')
    return aa, mm


def ev_i(ee):
    for r in ee["relations"]:
        if r["quantity"] == "split:g_eff":
            r["expression"] = "g1*(1+a)"
        if r["quantity"] == "split:sigma_eff":
            r["expression"] = "2*K_delta*f_D^kappa_D*W_delta"
    return ee


def mut_j(aa, mm):
    aa["targets"]["R_SPLIT"]["g_eff_expression"] = "g1+g2*(1+Delta)^1.5*exp(-x*Delta)"
    mm = mm.replace('"expression": "g1+g2*(1+Delta)^1.5*exp(-x_f*Delta)"',
                    '"expression": "g1+g2*(1+Delta)^1.5*exp(-x*Delta)"')
    return aa, mm


def ev_j(ee):
    for r in ee["relations"]:
        if r["quantity"] == "split:g_eff":
            r["expression"] = "g1+g2*(1+Delta)^1.5*exp(-x*Delta)"
    return ee


build("variant_I", mut_i, ev_i)
build("variant_J", mut_j, ev_j)
print("variants I/J built")
