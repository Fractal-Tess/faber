#!/usr/bin/env bash
# Replays a session file against a running Faber and prints a transcript.
# Commands are separated by blank lines; a command may span several lines.
# usage: FABER_KEY=<key> session/rec.sh session/compile.cmds > shots/compile.txt
export FABER=${FABER:-http://localhost:41500/api/v1}
export AUTH="Authorization: Bearer $FABER_KEY" JSON="Content-Type: application/json"
cmds=$(realpath "$1")
cd "$(dirname "$0")"
block=()
run() {
  [ ${#block[@]} -eq 0 ] && return
  printf '\x01%s\n' "${block[0]}"
  for l in "${block[@]:1}"; do printf '\x02%s\n' "$l"; done
  eval "$(printf '%s\n' "${block[@]}")" 2>&1
  block=()
}
while IFS= read -r line; do
  if [ -z "$line" ]; then run; else block+=("$line"); fi
done < "$cmds"
run
