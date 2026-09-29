#!/usr/bin/env python3
"""Select and validate simulator evidence for the commit just deployed."""
import argparse
import json
import os
from pathlib import Path
import re

from wait_deploy_gates import api, assess, fetch_runs


def select_run(runs, sha, repository):
    _, _, latest = assess(runs, sha, repository, [])
    run = latest.get("sim-evidence-unit-tests.yml")
    if run is None:
        return None  # Path-filtered workflow did not run for this commit.
    if run.get("status") != "completed" or run.get("conclusion") != "success":
        raise ValueError("Simulator evidence run is not successful")
    return run["id"]


def validate_payload(payload, item, sha, repository, run_id):
    expected_ref = f"https://github.com/{repository}/actions/runs/{run_id}"
    if (payload.get("gate_id") != "G-SIM-1"
            or payload.get("item_key") != item
            or payload.get("status") != "evidenced"
            or payload.get("evidence_ref") != expected_ref
            or payload.get("verified_by") != "ci:sim-evidence-unit-tests.yml"
            or not isinstance(payload.get("detail"), dict)
            or payload["detail"].get("source_sha") != sha):
        raise ValueError(f"Invalid or stale {item} evidence")


def publication_verdict(main_sha, target_sha, validating):
    """What this script may do about the readiness publication for `target_sha`.

    DEPLOY-VERAZ-01. This script decides a PUBLICATION, and the step that runs it
    sits AFTER the deployment step, so its failure must never be read as "the
    deploy failed". When `main` has moved on:

      * selecting (validating=False) — the deployment for `target_sha` already
        completed; the VPS HEAD is asserted by the deploy step's own post-check
        (`git rev-parse HEAD`), not by this GitHub ref. Nothing can be published
        for a superseded SHA, so the honest outcome is to SKIP the publication
        and say why, not to fail the delivery that already happened. A superseded
        publication is not a failed deploy.
      * validating (validating=True) — the caller is one step away from POSTING
        evidence. Publishing for a superseded SHA would stamp stale provenance
        onto the registry, so this stays a hard refusal.
    """
    if main_sha == target_sha:
        return "publish"
    return "refuse" if validating else "skip-superseded"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--validate-dir", type=Path)
    parser.add_argument("--run-id", type=int)
    args = parser.parse_args()
    sha, repository = os.environ["TARGET_SHA"], os.environ["GITHUB_REPOSITORY"]
    if (not re.fullmatch(r"[0-9a-f]{40}", sha)
            or not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository)):
        raise ValueError("Invalid deployment identity")
    main_sha = api(f"repos/{repository}/git/ref/heads/main")["object"]["sha"]
    verdict = publication_verdict(main_sha, sha, args.validate_dir is not None)
    if verdict == "refuse":
        raise ValueError("Deployment superseded; refusing stale readiness publication")
    if verdict == "skip-superseded":
        print(f"::warning::DEPLOY-VERAZ-01: main moved to {main_sha[:12]} while {sha[:12]} was being "
              "deployed. The deployment itself already completed (see 'Deploy to VPS via SSH'), so this "
              "is NOT a delivery failure; the readiness-evidence publication for this superseded SHA is "
              "skipped and will be published by the run that deploys the current main.")
        with open(os.environ["GITHUB_OUTPUT"], "a") as output:
            output.write("run_id=\n")
        return
    run_id = select_run(fetch_runs(repository, sha), sha, repository)
    if args.validate_dir is None:
        with open(os.environ["GITHUB_OUTPUT"], "a") as output:
            output.write(f"run_id={run_id or ''}\n")
        print(f"Evidence run: {run_id}" if run_id else "No simulator evidence run for this SHA")
        return
    if run_id is None or run_id != args.run_id:
        raise ValueError("Evidence run changed or missing")
    for item in ("unit_tests", "dep_tree"):
        path = args.validate_dir / f"readiness-evidence-{item.replace('_', '-')}" / "evidence-payload.json"
        if path.stat().st_size > 131072:
            raise ValueError("Evidence payload exceeds 128 KiB")
        validate_payload(json.loads(path.read_text()), item, sha, repository, run_id)
    print(f"Validated both evidence artifacts for {sha}, run {run_id}")


if __name__ == "__main__":
    main()
