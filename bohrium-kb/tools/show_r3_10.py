#!/usr/bin/env python3
"""Print top-12 + our attempts summary from judge_r3_10_data.json."""
import json, os, sys
sys.stdout.reconfigure(encoding="utf-8")
P = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\bohrium-kb\round3_prep\judge_r3_10_data.json"
d = json.load(open(P, encoding="utf-8"))

print("### TOP ATTEMPTS")
for t in d["top"]:
    sc = t.get("scorecard") or {}
    sd = json.dumps(t.get("scoringDetails"), ensure_ascii=False)
    rj = json.dumps(t.get("resultsJson"), ensure_ascii=False)
    print("aid=%s name=%s score=%s outcome=%s created=%s" % (
        t.get("id"), t.get("author_name"), t.get("score"), t.get("outcome"), t.get("createdAt")))
    print("   framework=%s model=%s agent=%s redacted=%s" % (
        t.get("agentFramework"), t.get("modelTag"), t.get("agentName"), t.get("answerRedacted")))
    print("   harbor=%s trace=%s tf=%s" % (sc.get("harbor_reward"), sc.get("trace_score"), sc.get("trace_factor")))
    print("   sd=%s" % sd[:400])
    print("   rj=%s" % rj[:250])
    print()

print("### OUR ATTEMPTS")
for t in d["our"]:
    sc = t.get("scorecard") or {}
    sd = json.dumps(t.get("scoringDetails"), ensure_ascii=False)
    print("aid=%s name=%s score=%s outcome=%s status=%s created=%s redacted=%s" % (
        t.get("id"), t.get("author_name"), t.get("score"), t.get("outcome"), t.get("status"),
        t.get("createdAt"), t.get("answerRedacted")))
    print("   harbor=%s trace=%s tf=%s" % (sc.get("harbor_reward"), sc.get("trace_score"), sc.get("trace_factor")))
    print("   sd=%s" % sd[:400])
    print()
