#!/bin/bash
# OMEGA GUARD — Pre-tool hook that blocks RULE 00 violations
# Runs BEFORE Claude Code writes/edits any file
# Exit 0 = allow, Exit 2 = BLOCK the action

FILE_PATH="$CLAUDE_TOOL_INPUT_FILE_PATH"

# Skip if no file path (non-file operations)
[ -z "$FILE_PATH" ] && exit 0

# RULE 00: Block mock/dummy/fake data injection into production code
if echo "$FILE_PATH" | grep -qE "(frontend/app|backend/.*src)"; then
  # Read the content being written
  CONTENT="$CLAUDE_TOOL_INPUT_CONTENT"
  
  if echo "$CONTENT" | grep -qiE "(mock|dummy|fake|placeholder|lorem ipsum|TODO: replace)"; then
    echo "🚫 OMEGA GUARD — RULE 00 VIOLATION BLOCKED"
    echo "   Attempted to write mock/dummy/fake data to production file: $FILE_PATH"
    echo "   Content matched: mock|dummy|fake|placeholder|lorem ipsum|TODO: replace"
    echo "   Doctrina Zero Mocks is ABSOLUTE. Use real data or show empty/loading/error state."
    exit 2
  fi

  # Block localhost hardcoding in frontend (R2/RULE 04)
  if echo "$FILE_PATH" | grep -q "frontend/"; then
    if echo "$CONTENT" | grep -qE "localhost:(8787|3000|5432|6379)"; then
      echo "🚫 OMEGA GUARD — RULE 04 VIOLATION BLOCKED"
      echo "   Attempted to hardcode localhost in frontend file: $FILE_PATH"
      echo "   Use NEXT_PUBLIC_* env vars instead. See R2 and RULE 04."
      exit 2
    fi
  fi
fi

# Allow all other operations
exit 0
