#!/usr/bin/env python3
"""Require successful main-push workflows for exactly the deployment SHA.

No third-party dependencies. A missing mandatory workflow is pending, never green.
Additional path-filtered validation workflows that ran for the SHA also gate deploy.
API errors fail closed; no access token or response bodies are printed.
"""
import argparse
import json
import os
from pathlib import Path
import re
import sys
import time
import urllib.parse
import urllib.request


def assess(runs, sha, repository, required):
    latest = {}
    for run in runs:
        if (run.get("head_sha") != sha or run.get("event") != "push"
                or run.get("head_branch") != "main"
                or run.get("repository", {}).get("full_name") != repository):
            continue
        path = run.get("path", "")
        if not path.startswith(".github/workflows/"):
            continue
        name = path.removeprefix(".github/workflows/")
        if name == "auto-deploy-vps.yml":
            continue
        previous = latest.get(name)
        # A newer failed/queued run must supersede an earlier green result.
        key = (run["id"], run.get("run_attempt", 1))
        if previous is None or key > (previous["id"], previous.get("run_attempt", 1)):
            latest[name] = run
    pending, failed = [], []
    for name in sorted(set(required) | set(latest)):
        run = latest.get(name)
        if run is None or run.get("status") != "completed":
            pending.append(name)
        elif run.get("conclusion") != "success":
            failed.append(f"{name}: {run.get('conclusion')} (run {run['id']})")
    return pending, failed, latest


def api(path):
    request = urllib.request.Request(
        "https://api.github.com/" + path,
        headers={"Authorization": "Bearer " + os.environ["GH_TOKEN"],
                 "Accept": "application/vnd.github+json",
                 "X-GitHub-Api-Version": "2022-11-28"},
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.load(response)


def fetch_runs(repository, sha):
    runs = []
    for page in range(1, 11):
        query = urllib.parse.urlencode({"head_sha": sha, "event": "push",
                                      "branch": "main", "per_page": 100, "page": page})
        data = api(f"repos/{repository}/actions/runs?{query}")
        batch = data["workflow_runs"]
        runs.extend(batch)
        if len(runs) >= data["total_count"]:
            return runs
        if not batch:
            break
    raise RuntimeError("Workflow results incomplete; refusing partial gate evidence")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--timeout", type=int, default=3600)
    args = parser.parse_args()
    repository, sha = os.environ["GITHUB_REPOSITORY"], os.environ["TARGET_SHA"]
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
        raise ValueError("Invalid repository")
    if not re.fullmatch(r"[0-9a-f]{40}", sha) or args.timeout < 0:
        raise ValueError("Invalid SHA or timeout")
    required = json.loads(Path(__file__).with_name("deploy-gates.json").read_text())
    if not required or len(required) != len(set(required)):
        raise ValueError("Invalid mandatory gate manifest")
    deadline = time.monotonic() + args.timeout
    while True:
        head = api(f"repos/{repository}/git/ref/heads/main")["object"]["sha"]
        if head != sha:
            raise RuntimeError("Target superseded by main; deploy the newer validated commit")
        pending, failed, latest = assess(fetch_runs(repository, sha), sha, repository, required)
        if failed:
            raise RuntimeError("Failed gates: " + "; ".join(failed))
        if not pending:
            print(f"PASS: {len(latest)} workflows for {sha}")
            summary = os.environ.get("GITHUB_STEP_SUMMARY")
            if summary:
                with open(summary, "a") as output:
                    output.write(f"\nValidated deployment SHA: `{sha}`\n\n")
                    for name, run in sorted(latest.items()):
                        output.write(f"- {name}: https://github.com/{repository}/actions/runs/{run['id']}\n")
            return
        print("Pending: " + ", ".join(pending), flush=True)
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise RuntimeError("Timed out waiting for mandatory deployment gates")
        time.sleep(min(30, remaining))


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        # HTTP error strings do not expose request headers. Avoid response bodies.
        print(f"::error::{type(error).__name__}: {error}", file=sys.stderr)
        sys.exit(1)
