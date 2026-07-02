#!/bin/sh
set -eu

wait_for_file() {
  wf_path=$1
  wf_secs=$2
  wf_n=0
  while [ "$wf_n" -lt "$wf_secs" ]; do
    if [ -s "$wf_path" ]; then
      return 0
    fi
    sleep 1
    wf_n=$((wf_n + 1))
  done
  [ -s "$wf_path" ]
}

: "${CITRINE_BIN:?CITRINE_BIN is required}"
: "${SENTINEL:?SENTINEL is required}"

OUT_DIR=$PWD/verify-out
mkdir -p "$OUT_DIR"

tmp=$(mktemp -d)
GHOSTTY_PID=""

cleanup() {
  if [ -n "$GHOSTTY_PID" ] && kill -0 "$GHOSTTY_PID" 2>/dev/null; then
    kill "$GHOSTTY_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT INT TERM

mkdir -p "$tmp/xdg/ghostty/themes"

"$CITRINE_BIN" export ghostty --palette "$SENTINEL" --out "$tmp/xdg/ghostty/themes/citrine-sentinel"

cat > "$tmp/run.sh" <<EOF
#!/bin/sh
"$CITRINE_BIN" probe --expect "$SENTINEL" --out "$tmp/result.json" --checks ansi,fg,bg
echo \$? > "$tmp/probe-exit"
exit 0
EOF
chmod +x "$tmp/run.sh"

cat > "$tmp/xdg/ghostty/config" <<EOF
theme = citrine-sentinel
command = $tmp/run.sh
confirm-close-surface = false
quit-after-last-window-closed = true
window-save-state = never
EOF

GHOSTTY_BIN=/Applications/Ghostty.app/Contents/MacOS/ghostty
if [ ! -x "$GHOSTTY_BIN" ]; then
  GHOSTTY_BIN=ghostty
fi

XDG_CONFIG_HOME="$tmp/xdg" "$GHOSTTY_BIN" +validate-config > "$OUT_DIR/ghostty-validate.log" 2>&1

XDG_CONFIG_HOME="$tmp/xdg" "$GHOSTTY_BIN" > "$OUT_DIR/ghostty-launch.log" 2>&1 &
GHOSTTY_PID=$!

wait_for_file "$tmp/result.json" 60 || true

if kill -0 "$GHOSTTY_PID" 2>/dev/null; then
  kill "$GHOSTTY_PID" 2>/dev/null || true
fi
GHOSTTY_PID=""

if [ ! -s "$tmp/result.json" ]; then
  echo "ghostty verify: no result produced" >&2
  exit 1
fi

cp "$tmp/result.json" "$OUT_DIR/ghostty-result.json"
cat "$tmp/result.json"

status=1
if grep -Eq '"pass"[[:space:]]*:[[:space:]]*true' "$tmp/result.json"; then
  status=0
elif [ -s "$tmp/probe-exit" ]; then
  status=$(cat "$tmp/probe-exit")
  if [ "$status" -eq 0 ]; then
    status=1
  fi
fi
exit "$status"
