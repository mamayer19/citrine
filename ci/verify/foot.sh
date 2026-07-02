#!/bin/sh
set -eu
. "$(dirname "$0")/common.sh"
mk_tmp
RESULT="$CITRINE_TMP/result.json"
if [ -z "${XDG_RUNTIME_DIR:-}" ]; then
  XDG_RUNTIME_DIR="$CITRINE_TMP/run"
  mkdir -p "$XDG_RUNTIME_DIR"
  chmod 700 "$XDG_RUNTIME_DIR"
  export XDG_RUNTIME_DIR
fi
"$CITRINE_BIN" export foot --palette "$SENTINEL" --out "$CITRINE_TMP/theme.ini"
printf 'include=%s\n' "$CITRINE_TMP/theme.ini" > "$CITRINE_TMP/foot.ini"
foot --config "$CITRINE_TMP/foot.ini" --check-config
weston --backend=headless-backend.so --socket=wl-citrine --idle-time=0 > "$CITRINE_TMP/weston.log" 2>&1 &
track_pid $!
sleep 2
WAYLAND_DISPLAY=wl-citrine foot --config "$CITRINE_TMP/foot.ini" sh -c "$(probe_cmd)" > "$CITRINE_TMP/foot.log" 2>&1 &
track_pid $!
wait_for_file "$RESULT" 60 || true
mkdir -p verify-out
cp "$CITRINE_TMP/weston.log" verify-out/weston.log 2>/dev/null || true
cp "$CITRINE_TMP/foot.log" verify-out/foot.log 2>/dev/null || true
finish "$RESULT"
