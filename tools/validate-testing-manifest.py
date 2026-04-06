#!/usr/bin/env python3

from __future__ import annotations

import json
import re
import sys
from pathlib import Path


def parse_just_recipes(justfile: Path) -> set[str]:
    recipes: set[str] = set()
    for line in justfile.read_text().splitlines():
        if not line or line[0].isspace():
            continue
        if line.startswith("[") or ":=" in line:
            continue
        match = re.match(r"^([A-Za-z0-9_-]+)(?:\s+.*)?:(?:\s.*)?$", line)
        if match:
            recipes.add(match.group(1))
    return recipes


def parse_workflow_jobs(workflow: Path) -> set[str]:
    jobs: set[str] = set()
    in_jobs = False
    for line in workflow.read_text().splitlines():
        if not in_jobs:
            if line == "jobs:":
                in_jobs = True
            continue
        if line and not line.startswith(" "):
            break
        match = re.match(r"^  ([A-Za-z0-9_-]+):\s*$", line)
        if match:
            jobs.add(match.group(1))
    return jobs


def check_required_substrings(path: Path, values: list[str], errors: list[str]) -> None:
    text = path.read_text()
    for value in values:
        if value not in text:
            errors.append(f"{path}: missing required snippet: {value}")


def main() -> int:
    repo_root = Path(__file__).resolve().parent.parent
    manifest_path = repo_root / "docs/testing-manifest.json"
    manifest = json.loads(manifest_path.read_text())

    errors: list[str] = []

    justfile = repo_root / "justfile"
    recipes = parse_just_recipes(justfile)
    for recipe in manifest["just_recipes"]:
        if recipe not in recipes:
            errors.append(f"{justfile}: missing required recipe: {recipe}")

    for rel_path, expected_jobs in manifest["workflow_jobs"].items():
        workflow_path = repo_root / rel_path
        actual_jobs = parse_workflow_jobs(workflow_path)
        for job in expected_jobs:
            if job not in actual_jobs:
                errors.append(f"{workflow_path}: missing required job id: {job}")

    for rel_path, snippets in manifest["workflow_snippets"].items():
        check_required_substrings(repo_root / rel_path, snippets, errors)

    for rel_path, commands in manifest["doc_commands"].items():
        check_required_substrings(repo_root / rel_path, commands, errors)

    for rel_path, snippets in manifest["doc_snippets"].items():
        check_required_substrings(repo_root / rel_path, snippets, errors)

    if errors:
        print("testing manifest validation failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print("testing manifest validation passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
