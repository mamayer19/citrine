#!/bin/sh
set -eu
. "$(dirname "$0")/common.sh"
mk_tmp
RESULT="$CITRINE_TMP/result.json"
"$CITRINE_BIN" verify-setup alacritty --palette "$SENTINEL" --dir "$CITRINE_TMP" --probe-cmd "$(probe_cmd)"
alacritty --config-file "$CITRINE_TMP/alacritty.toml" -e sh -c "$(probe_cmd)" > "$CITRINE_TMP/alacritty.log" 2>&1 &
track_pid $!
wait_for_file "$RESULT" 60 || true
mkdir -p verify-out
cp "$CITRINE_TMP/alacritty.log" verify-out/alacritty.log 2>/dev/null || true
finish "$RESULT"
