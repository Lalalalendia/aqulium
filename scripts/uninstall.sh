#!/usr/bin/env bash
set -euo pipefail

APP_PATH="$HOME/Applications/Aquilum.app"

if [[ -d "$APP_PATH" ]]; then
  rm -rf "$APP_PATH"
  printf 'Removed Aquilum from %s\n' "$APP_PATH"
else
  printf 'Aquilum is not installed in %s\n' "$HOME/Applications"
fi

printf 'Vaults, Markdown files, and Aquilum settings were left untouched.\n'
