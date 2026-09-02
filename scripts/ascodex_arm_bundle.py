"""Assemble an ARM v1.1 bundle from ASCodex solver evidence.

The Playground scorer reads ARM bundles: arm_manifest.json + src/reproduce.py +
execution/run.log + execution/results/* + characterization.json (the ONLY file
graders read for result_fidelity/output_coverage) + trace/trace.jsonl. This
tool deterministically packages an ASCodex solver workspace (trace.jsonl,
run.log, artifacts.json, analysis results) into that exact structure so the
software-guided trace lands in the scorer as-is instead of the agent guessing.

Read-only + local file assembly: no platform write here (upload happens via the
platform client after the operator reviews the bundle).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import sys
from pathlib import Path

SCHEMA_VERSION = "ascodex-arm-bundle/v1"


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def assemble(solver_ws: Path, out_dir: Path, paper: dict, expected_outputs: list[dict],
             characterization_deviations: list[dict], challenge_name: str | None = None) -> Path:
    """Package a solver workspace into an ARM v1.1 bundle directory.

    solver_ws layout expected (ASCodex convention):
      challenge/analysis/results.json        business result
      challenge/analysis/solve.py            entrypoint source (copied to src/)
      challenge/evidence/trace.jsonl         typed steps
      challenge/evidence/run.log             execution stdout
      challenge/evidence/artifacts.json      {artifacts: [{path, sha256}]}
      challenge/channel/*.json               probe responses (not bundled)

    out_dir receives:
      arm_manifest.json, characterization.json, trace/trace.jsonl,
      execution/run.log, execution/results/*, src/reproduce.py
    """
    if challenge_name:
        challenge_dir = solver_ws / challenge_name
        if not challenge_dir.is_dir():
            raise SystemExit(f"challenge dir not found: {challenge_dir}")
    else:
        challenge_dir = None
        for child in solver_ws.iterdir():
            if child.is_dir() and (child / "challenge.md").exists():
                challenge_dir = child
                break
        if challenge_dir is None:
            raise SystemExit(f"no challenge dir under {solver_ws}")

    evidence = challenge_dir / "evidence"
    analysis = challenge_dir / "analysis"
    for needed in ("trace.jsonl", "run.log", "artifacts.json"):
        if not (evidence / needed).exists():
            raise SystemExit(f"missing {evidence}/{needed}")

    # Clean target
    if out_dir.exists():
        shutil.rmtree(out_dir)
    (out_dir / "execution" / "results").mkdir(parents=True)
    (out_dir / "trace").mkdir(parents=True)
    (out_dir / "src").mkdir(parents=True)

    # 1. Copy execution evidence
    shutil.copy(evidence / "run.log", out_dir / "execution" / "run.log")
    shutil.copy(evidence / "trace.jsonl", out_dir / "trace" / "trace.jsonl")
    solve = analysis / "solve.py"
    if solve.exists():
        shutil.copy(solve, out_dir / "src" / "reproduce.py")
    # business results: copy each artifact listed in artifacts.json into execution/results
    artifacts_manifest = read_json(evidence / "artifacts.json")
    artifact_ids = []
    for art in artifacts_manifest.get("artifacts", []):
        raw = art["path"]
        # paths are workspace-root-relative (challenge/analysis/... or challenge/evidence/...)
        parts = raw.replace("\\", "/").split("/")
        # drop leading challenge dir if present -> analysis/results.json
        if len(parts) >= 2 and parts[0] in ("challenge", "ch-e2e-1", "ch-real-ff", "ch-e2e-1"):
            parts = parts[1:]
        src = solver_ws / Path(*parts)
        if not src.exists():
            src = challenge_dir / Path(*parts)
        if not src.exists():
            print(f"warning: artifact not found: {raw}", file=sys.stderr)
            continue
        dst_name = src.name
        shutil.copy(src, out_dir / "execution" / "results" / dst_name)
        artifact_ids.append({"id": dst_name, "path": f"execution/results/{dst_name}",
                             "checksum_sha256": sha256_file(src),
                             "format": "json" if dst_name.endswith(".json") else "other"})

    # 2. execution.artifacts + entrypoint
    run_log_path = out_dir / "execution" / "run.log"
    execution = {
        "entrypoint": "src/reproduce.py",
        "command": "python src/reproduce.py > execution/run.log 2>&1",
        "run.log": "execution/run.log",
        "ran_at": None,  # operator fills real timestamp on review
        "wall_time_s": None,
        "artifacts": artifact_ids,
    }

    # 3. characterization.json (grader's only input)
    characterization = {
        "deviations_from_paper": characterization_deviations,
        "envelope": [],
        "failure_modes": [],
    }
    (out_dir / "characterization.json").write_text(
        json.dumps(characterization, indent=1), encoding="utf-8")

    # 4. arm_manifest.json
    manifest = {
        "arm_version": "1.1",
        "paper": paper,
        "entrypoint": "src/reproduce.py",
        "expected_outputs": expected_outputs,
        "execution": execution,
        "trace": {"files": ["trace/trace.jsonl"],
                  "step_count": sum(1 for _ in (out_dir / "trace" / "trace.jsonl").open())},
        "characterization": {"path": "characterization.json"},
    }
    (out_dir / "arm_manifest.json").write_text(
        json.dumps(manifest, indent=1), encoding="utf-8")
    print(f"bundle assembled at {out_dir}")
    print(f"  trace steps: {manifest['trace']['step_count']}")
    print(f"  artifacts: {len(artifact_ids)}")
    return out_dir


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--solver-ws", required=True, type=Path)
    ap.add_argument("--challenge", default=None, help="challenge dir name under solver-ws")
    ap.add_argument("--out", required=True, type=Path)
    ap.add_argument("--paper-title", default="Reproduction")
    ap.add_argument("--paper-doi", default="")
    ap.add_argument("--expected-outputs", default="[]",
                    help="JSON array of {name, path} expected_outputs the grader checks")
    ap.add_argument("--deviations", default="[]",
                    help="JSON array of characterization deviations "
                         "{target, metric, actual_value, reference_value, score, tolerance?}")
    args = ap.parse_args()

    paper = {"title": args.paper_title, "doi": args.paper_doi}
    expected = json.loads(args.expected_outputs)
    deviations = json.loads(args.deviations)
    if not isinstance(expected, list) or not isinstance(deviations, list):
        raise SystemExit("expected-outputs and deviations must be JSON arrays")
    assemble(args.solver_ws, args.out, paper, expected, deviations, args.challenge)


if __name__ == "__main__":
    main()
