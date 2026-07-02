#!/bin/sh
set -eu
. "$(dirname "$0")/common.sh"
mk_tmp
RESULT="$CITRINE_TMP/result.json"
"$CITRINE_BIN" verify-setup rio --palette "$SENTINEL" --dir "$CITRINE_TMP" --probe-cmd "$(probe_cmd)"
XDG_CONFIG_HOME="$CITRINE_TMP/xdg"
export XDG_CONFIG_HOME
rio -e sh -c "$(probe_cmd)" > "$CITRINE_TMP/rio.log" 2>&1 &
track_pid $!
wait_for_file "$RESULT" 60 || true
mkdir -p verify-out
cp "$CITRINE_TMP/rio.log" verify-out/rio.log 2>/dev/null || true
finish "$RESULT"
