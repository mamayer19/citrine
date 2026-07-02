#!/bin/sh
set -eu
. "$(dirname "$0")/common.sh"
mk_tmp
RESULT="$CITRINE_TMP/result.json"
XDG_CONFIG_HOME="$CITRINE_TMP"
export XDG_CONFIG_HOME
mkdir -p "$CITRINE_TMP/rio/themes"
"$CITRINE_BIN" export rio --palette "$SENTINEL" --out "$CITRINE_TMP/rio/themes/citrine-sentinel.toml"
printf 'theme = "citrine-sentinel"\n' > "$CITRINE_TMP/rio/config.toml"
rio -e sh -c "$(probe_cmd)" > "$CITRINE_TMP/rio.log" 2>&1 &
track_pid $!
wait_for_file "$RESULT" 60 || true
mkdir -p verify-out
cp "$CITRINE_TMP/rio.log" verify-out/rio.log 2>/dev/null || true
finish "$RESULT"
