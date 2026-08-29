#!/usr/bin/env python3
"""Generate 3 answer variants for split coannihilation, fixed to paper values."""
import json
import os
import shutil
import sys

sys.stdout.reconfigure(encoding="utf-8")

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..",
                    "work", "mp-r-ab-uv-split-coann-6924985d"))
OUTS = os.path.join(ROOT, "outputs")

ans = json.load(open(os.path.join(OUTS, "final", "answer.json"), encoding="utf-8"))
der = json.load(open(os.path.join(OUTS, "evidence", "derivation.json"), encoding="utf-8"))
dm = open(os.path.join(OUTS, "DERIVATION.md"), encoding="utf-8").read()

def variant(tag, mutate):
    a = json.loads(json.dumps(ans))
    d = json.loads(json.dumps(der))
    m = dm
    a, d, m = mutate(a, d, m)
    vdir = os.path.join(ROOT, f"variant_{tag}")
    for sub in ("final", "evidence"):
        os.makedirs(os.path.join(vdir, sub), exist_ok=True)
    json.dump(a, open(os.path.join(vdir, "final", "answer.json"), "w", encoding="utf-8"), indent=2)
    json.dump(d, open(os.path.join(vdir, "evidence", "derivation.json"), "w", encoding="utf-8"), indent=2)
    open(os.path.join(vdir, "DERIVATION.md"), "w", encoding="utf-8").write(m)
    print(f"variant {tag} written to {vdir}")

def paper_uv(a, d, m):
    a["targets"]["R_UV"]["amplitude"]["polarization_sum_prefactor_over_C2"] = 0.6666666666666666
    a["targets"]["R_UV"]["cross_section"]["coefficient"] = "1/(24*pi)"
    for r in d["relations"]:
        if r["quantity"] == "uv:polarization_sum_prefactor_over_C2":
            r["expression"] = 0.6666666666666666
        if r["quantity"] == "uv:cross_section_coefficient":
            r["expression"] = "1/(24*pi)"
    m = m.replace('"quantity": "uv:polarization_sum_prefactor_over_C2",\n          "expression": 0.25',
                  '"quantity": "uv:polarization_sum_prefactor_over_C2",\n          "expression": 0.6666666666666666')
    m = m.replace('"quantity": "uv:cross_section_coefficient",\n          "expression": "1/(64*pi)"',
                  '"quantity": "uv:cross_section_coefficient",\n          "expression": "1/(24*pi)"')
    m = m.replace("with P = 1/4, i.e. polarization_sum_prefactor_over_C2 = 1/4",
                  "with P = 2/3, i.e. polarization_sum_prefactor_over_C2 = 2/3")
    m = m.replace("with coeff = 1/(64 pi)", "with coeff = 1/(24 pi)")
    return a, d, m

def fix_half(a, d, m):
    a["targets"]["R_SPLIT"]["equal_degenerate_limit_expression"] = "K_zero*f_D^kappa_D/2"
    for r in d["relations"]:
        if r["quantity"] == "split:equal_degenerate_limit":
            r["expression"] = "K_zero*f_D^kappa_D/2"
    return a, d, m

# Variant A: paper UV values + /2 fix, keep current R_SPLIT forms
variant("A", lambda a, d, m: fix_half(*paper_uv(a, d, m)))

# Variant B: A + fD_ratio expanded form + a_expression (3/2) form
def vb(a, d, m):
    a, d, m = fix_half(*paper_uv(a, d, m))
    a["targets"]["R_SPLIT"]["a_expression"] = "q*(1+Delta)^(3/2)*exp(-x_f*Delta)"
    a["targets"]["R_SPLIT"]["fD_ratio_expression"] = \
        "((K_zero/K_delta)*((a_zero/(1+a_zero)^2)/(a/(1+a)^2)))^(1/kappa_D)"
    for r in d["relations"]:
        if r["quantity"] == "split:a":
            r["expression"] = "q*(1+Delta)^(3/2)*exp(-x_f*Delta)"
        if r["quantity"] == "split:fD_ratio":
            r["expression"] = "((K_zero/K_delta)*((a_zero/(1+a_zero)^2)/(a/(1+a)^2)))^(1/kappa_D)"
    m = m.replace('"expression": "q*(1+Delta)^1.5*exp(-x_f*Delta)"',
                  '"expression": "q*(1+Delta)^(3/2)*exp(-x_f*Delta)"')
    m = m.replace('"expression": "((K_zero/K_delta)*(W_zero/W_delta))^(1/kappa_D)"',
                  '"expression": "((K_zero/K_delta)*((a_zero/(1+a_zero)^2)/(a/(1+a)^2)))^(1/kappa_D)"')
    return a, d, m
variant("B", vb)

# Variant C: paper UV + expanded B + no-factor-2 convention
def vc(a, d, m):
    a, d, m = vb(a, d, m)
    a["targets"]["R_SPLIT"]["sigma_eff_expression"] = "K_delta*f_D^kappa_D*a/(1+a)^2"
    a["targets"]["R_SPLIT"]["equal_degenerate_limit_expression"] = "K_zero*f_D^kappa_D/4"
    for r in d["relations"]:
        if r["quantity"] == "split:sigma_eff":
            r["expression"] = "K_delta*f_D^kappa_D*a/(1+a)^2"
        if r["quantity"] == "split:equal_degenerate_limit":
            r["expression"] = "K_zero*f_D^kappa_D/4"
    m = m.replace('"expression": "2*K_delta*f_D^kappa_D*a/(1+a)^2"',
                  '"expression": "K_delta*f_D^kappa_D*a/(1+a)^2"')
    m = m.replace('"expression": "K_zero*f_D^kappa_D/2"',
                  '"expression": "K_zero*f_D^kappa_D/4"')
    return a, d, m
variant("C", vc)
