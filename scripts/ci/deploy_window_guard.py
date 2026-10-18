#!/usr/bin/env python3
"""DEPLOY-WINDOW-GUARD-01 — refuse a merge while a deploy for `main` is in flight.

Mechanism for a rule that otherwise lives only in an operator's head: "do not
merge while a deploy is waiting on gates". On 2026-10-18 a merge landed inside
another deploy's gate window; the deploy then aborted with
`Target superseded by main` (correct — the guard refused a stale target), and the
evidence publication for that SHA was lost. The rule was right; the detection was
too late. This check moves the detection to the merge, where it can block.

Semantics: FAIL if any run of a DEPLOY workflow on `main` is not `completed`.
A deploy mid-flight must finish (or be stopped) before `main` moves again.

Exit 0 = window clear, 1 = a deploy is in flight. No third-party deps.
"""
import json
import os
import re
import sys
import urllib.parse
import urllib.request

# Workflows that mutate production when `main` moves. Keep in sync with the repo.
DEPLOY_WORKFLOWS = {
    ".github/workflows/auto-deploy-vps.yml",
    ".github/workflows/deploy.yml",
    ".github/workflows/vps-nginx-canonical-sync.yml",
}
IN_FLIGHT = {"queued", "in_progress", "waiting", "pending", "requested"}


def api(path, token):
    request = urllib.request.Request(
        "https://api.github.com/" + path,
        headers={"Authorization": "Bearer " + token,
                 "Accept": "application/vnd.github+json",
                 "X-GitHub-Api-Version": "2022-11-28"},
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.load(response)


def in_flight_runs(runs):
    """Deploy runs on `main` that have not reached a terminal state."""
    hits = []
    for run in runs:
        if run.get("path") not in DEPLOY_WORKFLOWS:
            continue
        if run.get("head_branch") != "main":
            continue
        if run.get("status") in IN_FLIGHT:
            hits.append(run)
    return hits


def main():
    token = os.environ.get("GH_TOKEN")
    repository = os.environ.get("GITHUB_REPOSITORY")
    if not token or not repository or not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
        print("::error::deploy-window-guard: missing GH_TOKEN/GITHUB_REPOSITORY")
        return 1
    query = urllib.parse.urlencode({"branch": "main", "per_page": 50})
    runs = api(f"repos/{repository}/actions/runs?{query}", token)["workflow_runs"]
    hits = in_flight_runs(runs)
    if hits:
        print("::error::DEPLOY-WINDOW-GUARD-01 — a deploy for main is in flight; do NOT merge.")
        for run in hits:
            url = run.get("html_url", "")
            print(f"  {run['path']}  id={run['id']}  status={run['status']}  "
                  f"head={run.get('head_sha', '')[:12]}  {url}")
        print("  Merging now moves `main` and supersedes that deploy (the target guard "
              "will abort it and its evidence publication is lost). Wait for it to close.")
        return 1
    print("DEPLOY-WINDOW-GUARD-01 — no deploy for main in flight; merge window is clear.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
