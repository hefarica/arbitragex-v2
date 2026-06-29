#!/usr/bin/env bash
# FAST deploy: ONE ssh connection. Streams the 3 files as a tar over the ssh pipe,
# untars on the VPS and runs the remote runner -- no per-file scp handshakes.
#   Usage: arbx_fastdeploy.sh <dryrun|safediff|apply> <env_path> [host] [deploy_dir]
set -euo pipefail
mode="${1:?mode (dryrun|safediff|apply) required}"
envp="${2:?env path required}"
host="${3:-arbx}"
dir="${4:-$(cd "$(dirname "$0")" && pwd)}"
cd "$dir"
for f in arbx_env_fragment.env arbx_env_upsert.sh arbx_remote.sh; do
  [ -f "$f" ] || { echo "missing: $dir/$f (run ExportEnvFragment first)" >&2; exit 2; }
done
# Single connection: tar -> ssh stdin -> untar -> run.
tar -czf - arbx_env_fragment.env arbx_env_upsert.sh arbx_remote.sh \
  | ssh -o BatchMode=yes -o ConnectTimeout=20 "$host" \
        "cd /tmp && tar xzf - && bash /tmp/arbx_remote.sh '$mode' '$envp'"
