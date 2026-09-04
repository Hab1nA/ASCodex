import json
import sys
from datetime import datetime, timezone
from pathlib import Path


TOOLS = Path(__file__).resolve().parents[1] / "bohrium-kb" / "tools"
sys.path.insert(0, str(TOOLS))

from submit_gate_audit import (  # noqa: E402
    audit_cadence,
    audit_channel,
    audit_identity,
    audit_model,
    audit_redline,
    audit_trace,
    main,
)


NOW = datetime(2026, 8, 27, 12, 0, tzinfo=timezone.utc)


def write_trace(root: Path) -> None:
    (root / "trace.jsonl").write_text(
        "\n".join(
            [
                json.dumps({"step_type": "tool_call", "tool_call_id": "c1", "timestamp": "2026-08-27T11:00:00Z"}),
                json.dumps({"step_type": "tool_result", "tool_call_id": "c1", "timestamp": "2026-08-27T11:00:01Z", "body": "run complete"}),
            ]
        )
        + "\n",
        encoding="utf-8",
    )


def test_all_six_gates_pass_for_clean_dry_run(tmp_path: Path, monkeypatch) -> None:
    (tmp_path / "answer.txt").write_text("derived value = 1.25\n", encoding="utf-8")
    (tmp_path / "run.log").write_text("tool executed successfully\n", encoding="utf-8")
    write_trace(tmp_path)
    monkeypatch.setenv("ASCODEX_MODEL", "GLM-5.3-Flash")
    payload = {
        "channel": "harbor_track",
        "script": False,
        "identity": {"name": "Friday-02", "status": "ACTIVE", "remaining": 5},
        "cadence": {
            "last_submits": ["2026-08-27T11:40:00Z"],
            "same_content_seconds": 4000,
            "content_sha256": "new",
            "last_content_sha256": "old",
        },
        "redline": {"artifacts": ["answer.txt"]},
        "trace": {"path": "trace.jsonl", "real_execution": True, "provenance_paths": ["run.log"]},
        "model": {"name": "GLM-5.3-Flash"},
    }
    assert audit_channel(payload)["passed"]
    assert audit_identity(payload)["passed"]
    assert audit_cadence(payload, NOW)["passed"]
    assert audit_redline(payload, tmp_path)["passed"]
    assert audit_trace(payload, tmp_path)["passed"]
    assert audit_model(payload)["passed"]


def test_each_high_risk_gate_blocks(tmp_path: Path, monkeypatch) -> None:
    (tmp_path / "answer.txt").write_text("harbor score from prior attempt\n", encoding="utf-8")
    (tmp_path / "run.log").write_text("run\n", encoding="utf-8")
    write_trace(tmp_path)
    monkeypatch.setenv("ASCODEX_MODEL", "GLM-5.3-Flash")
    assert not audit_channel({"channel": "judge_track", "script": True})["passed"]
    assert not audit_identity({"identity": {"name": "Friday-01", "status": "FROZEN", "remaining": 5}})["passed"]
    assert not audit_cadence(
        {
            "cadence": {
                "last_submits": ["2026-08-27T11:55:00Z", "2026-08-27T11:58:00Z"],
                "same_content_seconds": 60,
                "content_sha256": "same",
                "last_content_sha256": "same",
            }
        },
        NOW,
    )["passed"]
    assert not audit_redline({"redline": {"artifacts": ["answer.txt"]}}, tmp_path)["passed"]
    assert not audit_trace(
        {"trace": {"path": "trace.jsonl", "real_execution": False, "provenance_paths": ["run.log"]}},
        tmp_path,
    )["passed"]
    # 历史默认模型名 = 陈旧 provenance，直接拒
    assert not audit_model({"model": {"name": "DeepSeek-V4-Flash"}})["passed"]
    assert not audit_model({"model": {"name": "gpt-5.6-luna"}})["passed"]
    # 与声明模型不符 / 空 / unspecified 均拒
    assert not audit_model({"model": {"name": "unknown"}})["passed"]
    assert not audit_model({"model": {"name": ""}})["passed"]
    assert not audit_model({"model": {"name": "unspecified"}})["passed"]
    monkeypatch.delenv("ASCODEX_MODEL", raising=False)
    # 未声明 ASCODEX_MODEL 时：真实自报模型放行（无交叉核验），历史名仍拒
    assert audit_model({"model": {"name": "Some-Real-Model"}})["passed"]
    assert not audit_model({"model": {"name": "DeepSeek-V4-Flash"}})["passed"]


def test_structured_channel_and_trace_thresholds_are_enforced(tmp_path: Path) -> None:
    (tmp_path / "run.log").write_text("run\n", encoding="utf-8")
    write_trace(tmp_path)
    assert audit_channel({"channel": {"track": "harbor_track", "script_present": False, "expected_official": True}})["passed"]
    assert not audit_channel({"channel": {"track": "harbor_track", "script_present": False, "expected_official": False}})["passed"]
    payload = {
        "trace": {
            "path": "trace.jsonl",
            "real_execution": True,
            "provenance_paths": ["run.log"],
            "predicted_score": 69,
        }
    }
    assert not audit_trace(payload, tmp_path)["passed"]


def test_auditor_does_not_write_or_use_network(tmp_path: Path, monkeypatch) -> None:
    input_path = tmp_path / "audit.json"
    input_path.write_text("{}", encoding="utf-8")
    report_path = tmp_path / "report.json"
    monkeypatch.setattr(sys, "argv", ["submit_gate_audit.py", str(input_path), "--root", str(tmp_path), "--output", str(report_path)])
    assert main() == 2
    report = json.loads(report_path.read_text(encoding="utf-8"))
    assert report["dry_run"] is True
    assert report["network_used"] is False
    assert report["network_write_attempted"] is False
    assert report["decision"] == "blocked"
    assert report_path.is_file()


def test_requested_write_is_explicitly_rejected(tmp_path: Path, monkeypatch) -> None:
    input_path = tmp_path / "audit.json"
    input_path.write_text(json.dumps({"requested_write": True}), encoding="utf-8")
    monkeypatch.setattr(sys, "argv", ["submit_gate_audit.py", str(input_path), "--root", str(tmp_path)])
    assert main() == 2
