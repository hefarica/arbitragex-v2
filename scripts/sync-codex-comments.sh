#!/usr/bin/env bash
set -euo pipefail

REPO="${1:-hefarica/arbitragex-v2}"
PR="${2:-103}"
BOT="chatgpt-codex-connector[bot]"

OUT_DIR=".agents/github"
OUT_MD="$OUT_DIR/codex-pr-${PR}-comments.md"
OUT_TASKS="$OUT_DIR/codex-pr-${PR}-tasks.md"
OUT_JSON="$OUT_DIR/codex-pr-${PR}-comments.json"

mkdir -p "$OUT_DIR"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

echo "Syncing Codex comments for $REPO PR #$PR..."

# 1. Inline review comments
gh api "repos/$REPO/pulls/$PR/comments" --paginate > "$TMP_DIR/inline.json"

# 2. General conversation comments
gh api "repos/$REPO/issues/$PR/comments" --paginate > "$TMP_DIR/issue.json"

# 3. PR reviews
gh api "repos/$REPO/pulls/$PR/reviews" --paginate > "$TMP_DIR/reviews.json"

# Normalize into one JSON array
jq -s --arg bot "$BOT" '
  [
    .[0][]? | select(.user.login == $bot) | {
      source: "inline_review_comment",
      id,
      url: .html_url,
      created_at,
      updated_at,
      user: .user.login,
      path: (.path // null),
      line: (.line // null),
      start_line: (.start_line // null),
      side: (.side // null),
      body
    }
  ]
  +
  [
    .[1][]? | select(.user.login == $bot) | {
      source: "issue_conversation_comment",
      id,
      url: .html_url,
      created_at,
      updated_at,
      user: .user.login,
      path: null,
      line: null,
      start_line: null,
      side: null,
      body
    }
  ]
  +
  [
    .[2][]? | select(.user.login == $bot) | {
      source: "pr_review",
      id,
      url: .html_url,
      created_at,
      updated_at,
      user: .user.login,
      path: null,
      line: null,
      start_line: null,
      side: null,
      state,
      body: (.body // "")
    }
  ]
' "$TMP_DIR/inline.json" "$TMP_DIR/issue.json" "$TMP_DIR/reviews.json" > "$OUT_JSON"

COUNT="$(jq 'length' "$OUT_JSON")"
P1_COUNT="$(jq '[.[] | select(.body | test("P1"; "i"))] | length' "$OUT_JSON")"
P2_COUNT="$(jq '[.[] | select(.body | test("P2"; "i"))] | length' "$OUT_JSON")"
P3_COUNT="$(jq '[.[] | select(.body | test("P3"; "i"))] | length' "$OUT_JSON")"

cat > "$OUT_MD" <<EOF
# Codex Bot PR Comments Inbox

Repo: $REPO  
PR: #$PR  
Bot: $BOT  
Synced at: $(date -u +"%Y-%m-%dT%H:%M:%SZ")  

## Summary

- Total Codex comments: $COUNT
- P1 comments: $P1_COUNT
- P2 comments: $P2_COUNT
- P3 comments: $P3_COUNT

## Mandatory Rule

Before changing code, the agent must read this file completely and convert every Codex comment into a tracked task.

No P1 comment may be ignored.

---

EOF

jq -r '
.[] |
"## [" + (.source // "unknown") + "] " +
(if .path then .path else "PR conversation" end) +
(if .line then ":" + (.line|tostring) else "" end) + "\n\n" +
"- ID: " + (.id|tostring) + "\n" +
"- Created: " + (.created_at // "unknown") + "\n" +
"- Updated: " + (.updated_at // "unknown") + "\n" +
"- URL: " + (.url // "none") + "\n" +
(if .path then "- Path: " + .path + "\n" else "" end) +
(if .line then "- Line: " + (.line|tostring) + "\n" else "" end) +
"\n### Body\n\n" +
(.body // "") +
"\n\n---\n"
' "$OUT_JSON" >> "$OUT_MD"

cat > "$OUT_TASKS" <<EOF
# Codex Bot Resolution Matrix

Repo: $REPO  
PR: #$PR  
Synced at: $(date -u +"%Y-%m-%dT%H:%M:%SZ")  

| ID | Severity | Source | File | Line | Required action | Validation | Status |
|---|---|---|---|---:|---|---|---|
EOF

jq -r '
.[] |
"| " + (.id|tostring) +
" | " +
(if (.body | test("P1"; "i")) then "P1" elif (.body | test("P2"; "i")) then "P2" elif (.body | test("P3"; "i")) then "P3" else "INFO" end) +
" | " + (.source // "") +
" | " + (.path // "PR conversation") +
" | " + (if .line then (.line|tostring) else "" end) +
" | TODO: interpret Codex feedback and define fix | TODO: define validation | PENDING |"
' "$OUT_JSON" >> "$OUT_TASKS"

echo ""
echo "Codex comments synced:"
echo "- $OUT_MD"
echo "- $OUT_TASKS"
echo "- $OUT_JSON"
echo ""
echo "Total: $COUNT | P1: $P1_COUNT | P2: $P2_COUNT | P3: $P3_COUNT"

if [ "$P1_COUNT" -gt 0 ]; then
  echo "P1 comments detected. Agent must resolve P1 before any non-critical work."
fi
