#!/bin/bash
# OMEGA POST-HOOK — Runs AFTER every file write/edit
# Auto-formats TypeScript/JavaScript files with Prettier if available

FILE_PATH="$CLAUDE_TOOL_INPUT_FILE_PATH"

[ -z "$FILE_PATH" ] && exit 0

# Auto-format TS/JS/JSON files after edit
if echo "$FILE_PATH" | grep -qE "\.(ts|tsx|js|jsx|json|css)$"; then
  if command -v npx &> /dev/null && [ -f "$(dirname "$FILE_PATH")/../../package.json" ]; then
    npx prettier --write "$FILE_PATH" 2>/dev/null || true
  fi
fi

exit 0
