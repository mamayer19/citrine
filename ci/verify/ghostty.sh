#!/bin/sh
set -eu
. "$(dirname "$0")/common.sh"
mk_tmp
RESULT="$CITRINE_TMP/result.json"
"$CITRINE_BIN" verify-setup ghostty --palette "$SENTINEL" --dir "$CITRINE_TMP" --probe-cmd "$(probe_cmd)"
GHOSTTY_BIN=/Applications/Ghostty.app/Contents/MacOS/ghostty
if [ ! -x "$GHOSTTY_BIN" ]; then
  GHOSTTY_BIN=ghostty
fi
mkdir -p verify-out
XDG_CONFIG_HOME="$CITRINE_TMP/xdg" "$GHOSTTY_BIN" +validate-config > verify-out/ghostty-validate.log 2>&1
XDG_CONFIG_HOME="$CITRINE_TMP/xdg" "$GHOSTTY_BIN" > verify-out/ghostty-launch.log 2>&1 &
track_pid $!
wait_for_file "$RESULT" 60 || true
cp "$RESULT" verify-out/ghostty-result.json 2>/dev/null || true
finish "$RESULT"
