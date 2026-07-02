#!/bin/sh

CITRINE_PIDS=""

track_pid() {
  CITRINE_PIDS="$CITRINE_PIDS $1"
}

citrine_cleanup() {
  for citrine_pid in $CITRINE_PIDS; do
    kill "$citrine_pid" 2>/dev/null || true
  done
  if [ -n "${CITRINE_TMP:-}" ]; then
    rm -rf "$CITRINE_TMP"
  fi
}

mk_tmp() {
  CITRINE_TMP=$(mktemp -d)
  trap citrine_cleanup EXIT
  trap 'exit 129' HUP
  trap 'exit 130' INT
  trap 'exit 143' TERM
}

wait_for_file() {
  wff_path=$1
  wff_tries=$(($2 * 2))
  wff_i=0
  while [ "$wff_i" -lt "$wff_tries" ]; do
    if [ -s "$wff_path" ]; then
      return 0
    fi
    sleep 0.5
    wff_i=$((wff_i + 1))
  done
  [ -s "$wff_path" ]
}

probe_cmd() {
  printf 'exec "%s" probe --expect "%s" --out "%s" --checks ansi,fg,bg' "$CITRINE_BIN" "$SENTINEL" "$RESULT"
}

finish() {
  fin_path=$1
  mkdir -p verify-out
  if [ -f "$fin_path" ]; then
    cp "$fin_path" verify-out/result.json
    cat "$fin_path"
    if grep -q '"pass":[[:space:]]*true' "$fin_path"; then
      exit 0
    fi
  fi
  exit 1
}
