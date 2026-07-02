#!/bin/sh
set -eu
. "$(dirname "$0")/common.sh"
mk_tmp
RESULT="$CITRINE_TMP/result.json"
"$CITRINE_BIN" verify-setup wezterm --palette "$SENTINEL" --dir "$CITRINE_TMP" --probe-cmd "$(probe_cmd)"
wezterm --config-file "$CITRINE_TMP/wezterm.lua" ls-fonts > /dev/null
wezterm --config-file "$CITRINE_TMP/wezterm.lua" start > "$CITRINE_TMP/wezterm.log" 2>&1 &
track_pid $!
wait_for_file "$RESULT" 60 || true
mkdir -p verify-out
cp "$CITRINE_TMP/wezterm.log" verify-out/wezterm.log 2>/dev/null || true
finish "$RESULT"
