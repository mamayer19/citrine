#!/bin/sh
set -eu
. "$(dirname "$0")/common.sh"

hex_channel() {
  printf '%d' "0x$(printf '%s' "$1" | cut -c"$2"-"$(($2 + 1))")"
}

channel_ok() {
  co_diff=$(($1 - $2))
  if [ "$co_diff" -lt 0 ]; then
    co_diff=$((0 - co_diff))
  fi
  [ "$co_diff" -le 24 ]
}

mk_tmp
RESULT="$CITRINE_TMP/result.json"
SCHEME_DIR="$HOME/.local/share/konsole"
mkdir -p "$SCHEME_DIR"
"$CITRINE_BIN" export konsole --palette "$SENTINEL" --out "$SCHEME_DIR/Citrine Sentinel.colorscheme"
printf '[Appearance]\nColorScheme=Citrine Sentinel\n\n[General]\nName=citrine\n' > "$SCHEME_DIR/citrine.profile"
konsole --profile citrine -e sh -c "$(probe_cmd)" > "$CITRINE_TMP/konsole.log" 2>&1 &
track_pid $!
wait_for_file "$RESULT" 60 || true
mkdir -p verify-out
cp "$CITRINE_TMP/konsole.log" verify-out/konsole.log 2>/dev/null || true
if [ ! -s "$RESULT" ] || grep -q noreply "$RESULT"; then
  konsole --profile citrine -e sh -c "clear; sleep 8" > "$CITRINE_TMP/konsole-pixel.log" 2>&1 &
  track_pid $!
  sleep 4
  import -window root "$CITRINE_TMP/screen.png"
  cp "$CITRINE_TMP/screen.png" verify-out/screen.png 2>/dev/null || true
  PIXEL_HEX=$(convert "$CITRINE_TMP/screen.png" -resize 1x1 -depth 8 txt:- | sed -n 's/.*#\([0-9A-Fa-f]\{6\}\).*/\1/p' | head -n 1)
  if [ -n "$PIXEL_HEX" ] && channel_ok "$(hex_channel "$PIXEL_HEX" 1)" 16 && channel_ok "$(hex_channel "$PIXEL_HEX" 3)" 19 && channel_ok "$(hex_channel "$PIXEL_HEX" 5)" 23; then
    printf '{"pass":true,"mode":"pixel","actual":"#%s"}\n' "$PIXEL_HEX" > "$RESULT"
  else
    printf '{"pass":false,"mode":"pixel","actual":"#%s"}\n' "${PIXEL_HEX:-}" > "$RESULT"
  fi
fi
finish "$RESULT"
