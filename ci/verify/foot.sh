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
"$CITRINE_BIN" verify-setup foot --palette "$SENTINEL" --dir "$CITRINE_TMP" --probe-cmd "$(probe_cmd)"
foot --config "$CITRINE_TMP/foot.ini" --check-config
WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman \
  cage -- foot --config "$CITRINE_TMP/foot.ini" sh -c "$(probe_cmd)" > "$CITRINE_TMP/cage.log" 2>&1 &
track_pid $!
wait_for_file "$RESULT" 60 || true
mkdir -p verify-out
cp "$CITRINE_TMP/cage.log" verify-out/cage.log 2>/dev/null || true
finish "$RESULT"
