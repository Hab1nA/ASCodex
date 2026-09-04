#!/usr/bin/env python3
"""ascodex ZCode 迁移回归测试：提交门钩子 / 开题注入钩子 / trace_check / redline_scan。

全部命令均为参数列表调用（shell=False），输入全是本脚本内的固定字符串。
沙箱固定在 out/zcode-migration-test/sandbox/，不会删除本脚本自身。
用法：python tests/zcode-migration/test_all.py
"""
if __name__ != "__main__":
    # 本文件是独立脚本而非 pytest 用例；被 testpaths=tests 误收集时整模块跳过
    import pytest
    pytest.skip("standalone script: python tests/zcode-migration/test_all.py",
                allow_module_level=True)

import copy
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
T = REPO / "out" / "zcode-migration-test" / "sandbox"
GATE = REPO / ".zcode" / "hooks" / "submit-gate.js"
INJ = REPO / ".zcode" / "hooks" / "solve-prompt-inject.js"
TC = REPO / "work" / "_template" / "trace_check.py"
RS = REPO / "work" / "_template" / "redline_scan.py"
SB = REPO / "work" / "_template" / "submit_bundle.py"

results = []


def record(name, ok, detail=""):
    results.append((name, ok))
    print(("ok   - " if ok else "FAIL - ") + name + ("" if ok else f" ({detail})"))


def run_node(script, payload):
    env = dict(os.environ)
    env["ZCODE_PROJECT_DIR"] = str(T)
    proc = subprocess.run(
        ["node", str(script)],
        input=json.dumps(payload).encode("utf-8"),
        capture_output=True, env=env, timeout=30, shell=False)
    out = proc.stdout.decode("utf-8", "replace")
    err = proc.stderr.decode("utf-8", "replace")
    return proc.returncode, out, err


def run_py(script, argv):
    proc = subprocess.run(
        [sys.executable, str(script)] + list(argv),
        capture_output=True, timeout=60, shell=False)
    out = proc.stdout.decode("utf-8", "replace")
    err = proc.stderr.decode("utf-8", "replace")
    return proc.returncode, out, err


def bash(cmd):
    return {"tool_name": "Bash", "cwd": str(T), "tool_input": {"command": cmd}}


# ---------- 提交门 ----------
shutil.rmtree(T, ignore_errors=True)
(T / "work" / "ws-a").mkdir(parents=True)
(T / "work" / "ws-b").mkdir(parents=True)

code, out, err = run_node(GATE, bash("ls -la work/ws-a"))
record("非提交命令放行", code == 0, f"code={code} err={err[:160]}")

code, out, err = run_node(GATE, bash("curl https://play.bohrium.com/api/attempts -H 'x: 1'"))
record("只读 GET 放行", code == 0, f"code={code} err={err[:160]}")

code, out, err = run_node(GATE, bash("curl -fsSL https://play.bohrium.com/api/attempts"))
record("curl -fsSL 只读 GET 不误拦", code == 0, f"code={code} err={err[:160]}")

code, out, err = run_node(GATE, bash("Invoke-RestMethod https://play.bohrium.com/api/attempts"))
record("pwsh 原生 GET 侦察不误拦", code == 0, f"code={code} err={err[:160]}")

code, out, err = run_node(GATE, bash(
    "git diff HEAD -- work/_template/make_traces.py work/_template/submit_bundle.py"))
record("工具源文件路径提及不误判为调用", code == 0, f"code={code} err={err[:160]}")

code, out, err = run_node(GATE, bash("curl -X POST https://play.bohrium.com/api/attempts -d '{}'"))
record("无授权文件 POST 拒绝", code == 2 and "未找到任何提交授权" in err, f"code={code} err={err[:200]}")

code, out, err = run_node(GATE, bash("curl -d x=1 https://play.bohrium.com/api/attempts"))
record("curl -d 短旗标 POST 被拦截", code == 2, f"code={code} err={err[:160]}")

code, out, err = run_node(GATE, bash("Invoke-RestMethod https://play.bohrium.com/api/attempts -Method Post -Body x"))
record("Invoke-RestMethod -Method Post 被拦截", code == 2, f"code={code} err={err[:160]}")

code, out, err = run_node(GATE, bash("curl -X PATCH https://play.bohrium.com/api/attempts/9 -d x=1"))
record("PATCH 写动词被拦截", code == 2, f"code={code} err={err[:160]}")

code, out, err = run_node(GATE, bash("python work/_template/submit_bundle.py --challenge ch-1 --outcome partial"))
record("无授权 submit_bundle 拒绝", code == 2 and "提交授权" in err, f"code={code} err={err[:200]}")

code, out, err = run_node(GATE, bash("python3.12 work/_template/submit_bundle.py --challenge ch-1"))
record("python3.12 变体调用被拦截", code == 2, f"code={code} err={err[:160]}")

code, out, err = run_node(GATE, bash(str(T / "work" / "ws-a" / ".submit-authorized").join(["echo 5 > ", ""])))
record("Bash 触及授权文件拒绝(防自授权)", code == 2 and "只能由用户" in err, f"code={code} err={err[:160]}")

code, out, err = run_node(GATE, {"tool_name": "Write",
                                 "tool_input": {"file_path": str(T / "work" / "ws-a" / ".submit-authorized"),
                                                "content": "5"}})
record("Write 直写授权文件拒绝", code == 2 and "只能由用户" in err, f"code={code} err={err[:160]}")

code, out, err = run_node(GATE, bash("python work/_template/submit_bundle.py --challenge ch-1 --dry-run"))
record("submit_bundle --dry-run 放行", code == 0, f"code={code} err={err[:160]}")

code, out, err = run_node(GATE, bash("curl -X POST https://play.bohrium.com/api/attempts -d '{\"note\":\"dryrun\"}'"))
record("真实 POST 内嵌 dryrun 字样不豁免", code == 2, f"code={code} err={err[:160]}")

code, out, err = run_node(GATE, bash("python bohrium-kb/tools/submit_gate_audit.py --bundle work/ws-a"))
record("只读审计脚本放行", code == 0, f"code={code} err={err[:160]}")

code, out, err = run_node(GATE, bash(
    "curl -X POST https://play.bohrium.com/api/attempts -d x=1 && python bohrium-kb/tools/submit_gate_audit.py"))
record("POST 前置 + 审计后置链不豁免", code == 2, f"code={code} err={err[:160]}")

code, out, err = run_node(GATE, bash("echo submit_bundle.py is the uploader"))
record("仅提及不拦截", code == 0, f"code={code} err={err[:160]}")

# --- slug 定位授权：多会话并行各扣各的 ---
(T / "work" / "ws-a" / ".submit-authorized").write_text("2\n", encoding="utf-8")
(T / "work" / "ws-b" / ".submit-authorized").write_text("2\n", encoding="utf-8")

code, out, err = run_node(GATE, bash("curl -X POST https://play.bohrium.com/api/attempts -d x=1"))
record("无 slug + 多授权文件拒绝(防跨题误扣)", code == 2 and "多个授权文件" in err, f"code={code} err={err[:200]}")

code, out, err = run_node(GATE, bash("python work/ws-a/submit_bundle.py --challenge ch-1 --outcome partial"))
record("slug 定位放行并只扣 ws-a 2→1", code == 0 and "剩余 1" in err, f"code={code} err={err[:200]}")
record("ws-a 授权文件扣减正确", (T / "work" / "ws-a" / ".submit-authorized").read_text(encoding="utf-8").strip() == "1")
record("ws-b 授权文件不受影响", (T / "work" / "ws-b" / ".submit-authorized").read_text(encoding="utf-8").strip() == "2")

# 用户撤走 ws-a 授权后，仅剩 ws-b：无 slug 命令回退恰一原则
os.remove(T / "work" / "ws-a" / ".submit-authorized")

code, out, err = run_node(GATE, bash("curl --data y=2 https://play.bohrium.com/api/attempts/9/bundle"))
record("无 slug 时回退恰一原则放行 2→1", code == 0 and "剩余 1" in err, f"code={code} err={err[:160]}")

code, out, err = run_node(GATE, bash("curl -X POST https://play.bohrium.com/api/attempts -d z=3"))
record("无 slug 第二次放行 1→0 并移除 ws-b", code == 0 and "剩余 0" in err, f"code={code} err={err[:160]}")
record("ws-b 扣尽后授权文件已删除", not (T / "work" / "ws-b" / ".submit-authorized").exists())

code, out, err = run_node(GATE, bash("python work/ws-a/submit_bundle.py --challenge ch-1"))
record("slug 指向无授权的 ws-a 拒绝", code == 2 and "ws-a" in err, f"code={code} err={err[:200]}")

# ---------- 开题注入 ----------
code, out, err = run_node(INJ, {"prompt": "开始解题 challenge=ch-1 workspace=work/ws-a identity=x auth=dry-run"})
record("触发词'开始解题'注入", code == 0 and "ascodex-solve" in out and "提交纪律" in out,
       f"code={code} out={out[:160]}")

code, out, err = run_node(INJ, {"prompt": "解这道题 https://play.bohrium.com/#challenge/ch-9"})
record("challenge URL 开场同样注入", code == 0 and "ascodex-solve" in out, f"code={code} out={out[:160]}")

code, out, err = run_node(INJ, {"prompt": "帮我看看昨天的提交"})
record("非触发词静默放行", code == 0 and out == "", f"code={code} out={out[:160]}")

# ---------- trace_check ----------
W = T / "fixtures"
(W / "outputs").mkdir(parents=True, exist_ok=True)
(W / "execution").mkdir(parents=True, exist_ok=True)
(W / "outputs" / "answer.txt").write_text("4.013560313\n", encoding="utf-8")
(W / "execution" / "run.log").write_text(
    "$ python solve.py\n[OUT] computed value 4.013560313\n"
    "$ python verify.py\n[OK] within tolerance\n", encoding="utf-8")

thought1 = ("读题面并逐字翻译评分契约：本题按字段相对误差打分 tolerance=0.02，"
            "先建立自建 verifier，确定保留名清单、输出 schema 与符号保留要求，再动手计算。")
thought2 = ("分析运行输出：computed value 4.013560313 落在自建 verifier 的参考区间内，"
            "下一步把结果写入 outputs/answer.txt 并做一次干净重跑，把命令与 stdout 固定进 run.log 证据链。")
thought3 = ("收尾检查：确认 outputs/answer.txt 内容与最终一次运行的 stdout 一致，"
            "核对 artifact 相对路径、时间戳顺序与 cost 下限，准备转录 trace 并跑 trace_check。")
good_rows = [
    {"step_order": 1, "step_id": "s01", "step_type": "thought", "body": thought1,
     "timestamp": "2026-08-17T11:30:00Z", "duration_s": 1.2, "cost_usd": 0.005, "tokens": 10},
    {"step_order": 2, "step_id": "s02", "step_type": "tool_call", "tool_name": "shell",
     "tool_args": {"command": "python solve.py"}, "tool_call_id": "tc02",
     "timestamp": "2026-08-17T11:30:05Z", "duration_s": 0.1, "cost_usd": 0.003, "tokens": 0},
    {"step_order": 3, "step_id": "s03", "step_type": "tool_result", "tool_call_id": "tc02",
     "body": "[OUT] computed value 4.013560313",
     "timestamp": "2026-08-17T11:30:15Z", "duration_s": 10.0, "cost_usd": 0.004, "tokens": 0},
    {"step_order": 4, "step_id": "s04", "step_type": "thought", "body": thought2,
     "timestamp": "2026-08-17T11:30:20Z", "duration_s": 1.1, "cost_usd": 0.002, "tokens": 5},
    {"step_order": 5, "step_id": "s05", "step_type": "tool_call", "tool_name": "shell",
     "tool_args": {"command": "python verify.py"}, "tool_call_id": "tc05",
     "timestamp": "2026-08-17T11:30:25Z", "duration_s": 0.1, "cost_usd": 0.001, "tokens": 0},
    {"step_order": 6, "step_id": "s06", "step_type": "tool_result", "tool_call_id": "tc05",
     "body": "[OK] within tolerance",
     "timestamp": "2026-08-17T11:30:26Z", "duration_s": 1.0, "cost_usd": 0.0, "tokens": 0},
    {"step_order": 7, "step_id": "s07", "step_type": "thought", "body": thought3,
     "timestamp": "2026-08-17T11:30:30Z", "duration_s": 1.0, "cost_usd": 0.001, "tokens": 4},
    {"step_order": 8, "step_id": "s08", "step_type": "artifact", "artifact_path": "outputs/answer.txt",
     "body": "sha256:fake", "timestamp": "2026-08-17T11:30:35Z", "duration_s": 0.2, "cost_usd": 0.0, "tokens": 0},
]


def write_trace(name, rows):
    path = W / name
    path.write_text("\n".join(json.dumps(r, ensure_ascii=False) for r in rows) + "\n", encoding="utf-8")
    return str(path)


RUNLOG = str(W / "execution" / "run.log")

tr = write_trace("trace.jsonl", good_rows)
code, out, err = run_py(TC, [tr, "--run-log", RUNLOG, "--root", str(W)])
record("trace_check 合法 trace PASS", code == 0, f"code={code} out={out[:200]} err={err[:160]}")

bad = copy.deepcopy(good_rows)
del bad[2]
for i, row in enumerate(bad, 1):
    row["step_order"] = i
code, out, err = run_py(TC, [write_trace("bad_pairing.jsonl", bad), "--run-log", RUNLOG, "--root", str(W)])
record("trace_check 配对缺失 FAIL", code == 1 and "tool_result" in out, f"code={code} out={out[:200]}")

bad = copy.deepcopy(good_rows)
bad[0]["timestamp"], bad[2]["timestamp"] = bad[2]["timestamp"], bad[0]["timestamp"]
code, out, err = run_py(TC, [write_trace("bad_ts.jsonl", bad), "--run-log", RUNLOG, "--root", str(W)])
record("trace_check 时间戳回退 FAIL", code == 1 and "递减" in out, f"code={code} out={out[:200]}")

bad = copy.deepcopy(good_rows)
for row in bad:
    if row["step_type"] == "tool_result":
        row["body"] = "这段输出并未出现在 run.log 里的伪造文本 abcdefg"
code, out, err = run_py(TC, [write_trace("bad_anchor.jsonl", bad), "--run-log", RUNLOG, "--root", str(W)])
record("trace_check 锚定缺失 FAIL", code == 1 and "锚定" in out, f"code={code} out={out[:200]}")

bad = copy.deepcopy(good_rows)
bad[3]["body"] = "Per Maliar et al. the reference value follows Paper [1] Table 3, adopt directly."
code, out, err = run_py(TC, [write_trace("bad_cite.jsonl", bad), "--run-log", RUNLOG, "--root", str(W)])
record("trace_check 论文引用 FAIL", code == 1 and "论文引用" in out, f"code={code} out={out[:200]}")

# ---------- redline_scan ----------
clean = W / "clean_ws"
clean.mkdir(parents=True, exist_ok=True)
(clean / "a.txt").write_text("从题面推导：value = k*T/m，纯推导无外部情报\n", encoding="utf-8")
code, out, err = run_py(RS, [str(clean)])
record("redline_scan 干净目录 CLEAN", code == 0 and "CLEAN" in out, f"code={code} out={out[:200]}")

dirty = W / "dirty_ws"
dirty.mkdir(parents=True, exist_ok=True)
(dirty / "a.txt").write_text("harbor_reward 0.92, attempt 29181 credited\n", encoding="utf-8")
(dirty / "b.md").write_text("对手用了 160 权重，榜单第 3\n", encoding="utf-8")
code, out, err = run_py(RS, [str(dirty)])
hit_lines = out.count("\n  ")
record("redline_scan 污染目录 DIRTY", code == 1 and hit_lines >= 3, f"code={code} hits={hit_lines} out={out[:200]}")

custom = W / "custom_ws"
custom.mkdir(parents=True, exist_ok=True)
(custom / "redline_terms.txt").write_text("内部代号-xyz\n", encoding="utf-8")
(custom / "a.txt").write_text("这里提到 内部代号-xyz 应命中\n", encoding="utf-8")
code, out, err = run_py(RS, [str(custom)])
record("redline_scan 自定义词条生效", code == 1 and "内部代号-xyz" in out, f"code={code} out={out[:200]}")

diag = W / "diag_ws"
diag.mkdir(parents=True, exist_ok=True)
(diag / "a.txt").write_text("干净正文\n", encoding="utf-8")
(diag / "diagnostics").mkdir(exist_ok=True)
(diag / "diagnostics" / "intel.txt").write_text("平台情报区：harbor_reward 应被跳过\n", encoding="utf-8")
code, out, err = run_py(RS, [str(diag)])
record("redline_scan 跳过 diagnostics/ 平台情报区", code == 0 and "CLEAN" in out, f"code={code} out={out[:200]}")

# ---------- submit_bundle fail-closed（模板当前无 trace） ----------
proc = subprocess.run(
    [sys.executable, str(SB), "--challenge", "x", "--dry-run"],
    capture_output=True, timeout=60, shell=False)
out = (proc.stdout + proc.stderr).decode("utf-8", "replace")
record("submit_bundle 无 trace 时 fail-closed", proc.returncode != 0 and "FAIL-CLOSED" in out,
       f"code={proc.returncode} out={out[:200]}")

# ---------- 汇总 ----------
failed = [name for name, ok in results if not ok]
print(f"\n==== {len(results) - len(failed)}/{len(results)} passed ====")
if failed:
    print("FAILED:", failed)
    sys.exit(1)
