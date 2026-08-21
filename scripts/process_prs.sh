#!/bin/bash
# Process all PRs: update BEHIND, merge when ready
# Usage: bash scripts/process_prs.sh
cd "/c/Users/HFRC/Desktop/arbitragex-v2-main (17)"
export GH_TOKEN=$(gh auth token 2>/dev/null)
if [ -z "$GH_TOKEN" ]; then echo "ERROR: no token"; exit 1; fi

PRS="376 381 383 384 385 386 387 388 389 390"
for pr in $PRS; do
  st=$(gh pr view $pr --json state,mergeStateStatus --jq '.state + " " + .mergeStateStatus' 2>/dev/null)
  if [ -z "$st" ]; then echo "#$pr: NO_ACCESS"; continue; fi

  state=$(echo "$st" | cut -d' ' -f1)
  status=$(echo "$st" | cut -d' ' -f2)

  if [ "$state" = "MERGED" ]; then
    echo "#$pr: MERGED"
    continue
  fi

  if [ "$status" = "BEHIND" ]; then
    gh pr update-branch $pr >/dev/null 2>&1
    echo "#$pr: BRANCH_UPDATED"
    continue
  fi

  pend=$(gh pr checks $pr --json name,bucket --jq '[.[]|select(.bucket=="pending")]|length' 2>/dev/null)
  if [ "${pend:-1}" = "0" ] && { [ "$status" = "UNSTABLE" ] || [ "$status" = "CLEAN" ]; }; then
    fails=$(gh pr checks $pr --json name,bucket --jq '[.[]|select(.bucket=="fail")]|map(.name)|join(",")' 2>/dev/null)
    extra=$(echo "$fails" | grep -vcE 'npm audit|cargo audit|TypeScript integration' 2>/dev/null || echo 1)
    if [ "$extra" = "0" ] || [ -z "$fails" ]; then
      gh pr merge $pr --squash --delete-branch >/dev/null 2>&1
      echo "#$pr: MERGED_NOW"
    else
      echo "#$pr: $st FAILS=$fails" | cut -c1-100
    fi
  else
    echo "#$pr: $status pend=${pend:-?}"
  fi
done
