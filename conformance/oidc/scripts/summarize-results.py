#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


OP_MODULE_RE = re.compile(r"^\s*([A-Za-z0-9._-]+)\s+(PASSED|FAILED|WARNING|REVIEW)\s*$")
OP_TOTALS_RE = re.compile(r"Overall totals:\s*ran\s+(\d+)\s+test modules\.", re.IGNORECASE)
OP_CONDITIONS_RE = re.compile(
    r"Conditions:\s*(\d+)\s+successes,\s*(\d+)\s+failures,\s*(\d+)\s+warnings\.",
    re.IGNORECASE,
)


@dataclass
class SurfaceSummary:
    name: str
    status: str
    data: dict[str, Any]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Summarize OIDC conformance artifacts.")
    parser.add_argument("--artifact-root", required=True, help="Root artifact directory.")
    parser.add_argument("--surface", choices=("op", "rp", "both"), default="both")
    parser.add_argument(
        "--run-status",
        choices=("success", "failure", "cancelled", "skipped"),
        default="success",
        help="Workflow step outcome for the conformance run.",
    )
    parser.add_argument("--markdown-out", required=True, help="Path to write markdown summary.")
    parser.add_argument("--json-out", required=True, help="Path to write JSON summary.")
    parser.add_argument(
        "--github-output",
        help="Optional GitHub output file path for key=value outputs.",
    )
    return parser.parse_args()


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def relpath(path: Path, root: Path) -> str:
    return path.relative_to(root).as_posix()


def format_duration_ms(duration_ms: int | None) -> str | None:
    if duration_ms is None:
        return None
    seconds = duration_ms / 1000
    if seconds < 60:
        return f"{seconds:.1f}s"
    minutes = int(seconds // 60)
    remaining = seconds - (minutes * 60)
    return f"{minutes}m {remaining:.1f}s"


def parse_metadata(path: Path) -> dict[str, str]:
    if not path.exists():
        return {}
    metadata: dict[str, str] = {}
    for line in read_text(path).splitlines():
        if "=" not in line:
            continue
        key, value = line.split("=", 1)
        metadata[key.strip()] = value.strip()
    return metadata


def parse_op(root: Path) -> SurfaceSummary:
    op_dir = root / "op"
    if not op_dir.exists():
        return SurfaceSummary("op", "not_run", {})

    metadata = parse_metadata(op_dir / "metadata.txt")
    run_log = op_dir / "run.log"
    zip_files = sorted(op_dir.glob("*.zip"))
    latest_export = max(zip_files, key=lambda path: path.stat().st_mtime) if zip_files else None
    result: dict[str, Any] = {
        "metadata": metadata,
        "latest_export": relpath(latest_export, root) if latest_export else None,
        "exports": [relpath(path, root) for path in zip_files],
        "run_log": relpath(run_log, root) if run_log.exists() else None,
    }

    statuses: dict[str, list[str]] = {"FAILED": [], "WARNING": [], "REVIEW": []}
    status = "unknown"
    if run_log.exists():
        text = read_text(run_log)
        module_matches = OP_MODULE_RE.findall(text)
        for name, module_status in module_matches:
            if module_status in statuses:
                statuses[module_status].append(name)

        totals_match = None
        for match in OP_TOTALS_RE.finditer(text):
            totals_match = match
        if totals_match:
            result["modules"] = int(totals_match.group(1))

        conditions_match = None
        for match in OP_CONDITIONS_RE.finditer(text):
            conditions_match = match
        if conditions_match:
            result["conditions"] = {
                "successes": int(conditions_match.group(1)),
                "failures": int(conditions_match.group(2)),
                "warnings": int(conditions_match.group(3)),
            }

        result["failed_modules"] = statuses["FAILED"]
        result["warning_modules"] = statuses["WARNING"]
        result["review_modules"] = statuses["REVIEW"]

        if result.get("conditions"):
            status = "failed" if result["conditions"]["failures"] > 0 else "passed"
        elif statuses["FAILED"]:
            status = "failed"
        elif zip_files:
            status = "passed"

    return SurfaceSummary("op", status, result)


def parse_rp(root: Path) -> SurfaceSummary:
    rp_dir = root / "rp"
    if not rp_dir.exists():
        return SurfaceSummary("rp", "not_run", {})

    result: dict[str, Any] = {}
    results_json = rp_dir / "results.json"
    last_run = rp_dir / "test-results" / ".last-run.json"
    html_report = rp_dir / "playwright-report" / "index.html"
    run_log = rp_dir / "run.log"

    result["results_json"] = relpath(results_json, root) if results_json.exists() else None
    result["html_report"] = relpath(html_report, root) if html_report.exists() else None
    result["run_log"] = relpath(run_log, root) if run_log.exists() else None

    status = "unknown"
    if results_json.exists():
        report = json.loads(read_text(results_json))
        stats = report.get("stats", {})
        result["stats"] = {
            "passed": int(stats.get("expected", 0)),
            "failed": int(stats.get("unexpected", 0)),
            "flaky": int(stats.get("flaky", 0)),
            "skipped": int(stats.get("skipped", 0)),
            "duration_ms": int(stats.get("duration", 0)),
        }
        status = "failed" if result["stats"]["failed"] > 0 else "passed"
    elif last_run.exists():
        last = json.loads(read_text(last_run))
        result["last_run"] = last
        status = "failed" if last.get("status") == "failed" else "passed"

    return SurfaceSummary("rp", status, result)


def overall_status(run_status: str, requested_surface: str, surfaces: list[SurfaceSummary]) -> str:
    if run_status == "cancelled":
        return "cancelled"
    if run_status == "skipped":
        return "skipped"
    active = [surface for surface in surfaces if requested_surface in ("both", surface.name)]
    if any(surface.status == "failed" for surface in active):
        return "failed"
    if run_status == "failure":
        return "failed"
    if all(surface.status in ("passed", "not_run") for surface in active):
        return "passed"
    return "incomplete"


def render_markdown(run_status: str, requested_surface: str, overall: str, surfaces: list[SurfaceSummary]) -> str:
    lines = [
        "## OIDC Protocol Compliance",
        f"- Overall: `{overall}`",
        f"- Surface: `{requested_surface}`",
        f"- Runner outcome: `{run_status}`",
    ]

    for surface in surfaces:
        if requested_surface not in ("both", surface.name):
            continue

        lines.append(f"### {surface.name.upper()}")
        lines.append(f"- Status: `{surface.status}`")

        if surface.name == "op" and surface.data:
            conditions = surface.data.get("conditions")
            if conditions:
                lines.append(
                    "- Conditions: "
                    f"{conditions['successes']} successes, "
                    f"{conditions['failures']} failures, "
                    f"{conditions['warnings']} warnings"
                )
            if "modules" in surface.data:
                lines.append(f"- Modules: {surface.data['modules']}")
            if surface.data.get("review_modules"):
                lines.append(
                    "- Review modules: "
                    + ", ".join(f"`{name}`" for name in surface.data["review_modules"][:10])
                )
            if surface.data.get("failed_modules"):
                lines.append(
                    "- Failed modules: "
                    + ", ".join(f"`{name}`" for name in surface.data["failed_modules"][:10])
                )
            if surface.data.get("latest_export"):
                lines.append(f"- Latest export: `{surface.data['latest_export']}`")
            if surface.data.get("run_log"):
                lines.append(f"- Run log: `{surface.data['run_log']}`")

        if surface.name == "rp" and surface.data:
            stats = surface.data.get("stats")
            if stats:
                lines.append(
                    "- Playwright: "
                    f"{stats['passed']} passed, "
                    f"{stats['failed']} failed, "
                    f"{stats['flaky']} flaky, "
                    f"{stats['skipped']} skipped"
                )
                duration = format_duration_ms(stats.get("duration_ms"))
                if duration:
                    lines.append(f"- Duration: {duration}")
            elif surface.data.get("last_run"):
                lines.append(f"- Last run status: `{surface.data['last_run'].get('status', 'unknown')}`")
            if surface.data.get("results_json"):
                lines.append(f"- JSON report: `{surface.data['results_json']}`")
            if surface.data.get("html_report"):
                lines.append(f"- HTML report: `{surface.data['html_report']}`")
            if surface.data.get("run_log"):
                lines.append(f"- Run log: `{surface.data['run_log']}`")

        if surface.status == "not_run":
            lines.append("- No artifacts were produced for this surface.")

    return "\n".join(lines) + "\n"


def write_outputs(path: str | None, outputs: dict[str, str]) -> None:
    if not path:
        return
    with open(path, "a", encoding="utf-8") as handle:
        for key, value in outputs.items():
            handle.write(f"{key}={value}\n")


def main() -> int:
    args = parse_args()
    artifact_root = Path(args.artifact_root).resolve()
    artifact_root.mkdir(parents=True, exist_ok=True)

    op_summary = parse_op(artifact_root)
    rp_summary = parse_rp(artifact_root)
    summaries = [op_summary, rp_summary]
    overall = overall_status(args.run_status, args.surface, summaries)

    markdown = render_markdown(args.run_status, args.surface, overall, summaries)
    summary_json = {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "surface": args.surface,
        "run_status": args.run_status,
        "overall_status": overall,
        "op": {"status": op_summary.status, **op_summary.data},
        "rp": {"status": rp_summary.status, **rp_summary.data},
    }

    markdown_out = Path(args.markdown_out)
    markdown_out.parent.mkdir(parents=True, exist_ok=True)
    markdown_out.write_text(markdown, encoding="utf-8")

    json_out = Path(args.json_out)
    json_out.parent.mkdir(parents=True, exist_ok=True)
    json_out.write_text(json.dumps(summary_json, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    write_outputs(
        args.github_output,
        {
            "overall_status": overall,
            "op_status": op_summary.status,
            "rp_status": rp_summary.status,
        },
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
