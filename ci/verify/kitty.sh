#!/bin/sh
set -eu
. "$(dirname "$0")/common.sh"
mk_tmp
RESULT="$CITRINE_TMP/result.json"
"$CITRINE_BIN" export kitty --palette "$SENTINEL" --out "$CITRINE_TMP/theme.conf"
printf 'include theme.conf\nallow_remote_control yes\n' > "$CITRINE_TMP/kitty.conf"
if ! kitty --config "$CITRINE_TMP/kitty.conf" --debug-config 2>/dev/null | grep -qi color0; then
  grep -qi color0 "$CITRINE_TMP/theme.conf"
fi
kitty --config "$CITRINE_TMP/kitty.conf" --listen-on "unix:$CITRINE_TMP/kitty.sock" sh -c "$(probe_cmd)" > "$CITRINE_TMP/kitty.log" 2>&1 &
track_pid $!
wait_for_file "$RESULT" 60 || true
mkdir -p verify-out
kitten @ --to "unix:$CITRINE_TMP/kitty.sock" get-colors > verify-out/kitty-get-colors.txt 2>&1 || true
cp "$CITRINE_TMP/kitty.log" verify-out/kitty.log 2>/dev/null || true
finish "$RESULT"
