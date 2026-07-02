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
PROFILE_DIR="$HOME/Library/Application Support/iTerm2/DynamicProfiles"
PROFILE_FILE="$PROFILE_DIR/citrine-sentinel.json"
LAUNCHED=0

cleanup() {
  if [ "$LAUNCHED" -eq 1 ] && [ "${CI:-}" = "true" ]; then
    pkill -x iTerm2 >/dev/null 2>&1 || true
  fi
  rm -f "$PROFILE_FILE"
}
trap cleanup EXIT INT TERM

cat > "$tmp/run.sh" <<EOF
#!/bin/sh
"$CITRINE_BIN" probe --expect "$SENTINEL" --out "$tmp/result.json" --checks ansi,fg,bg
echo \$? > "$tmp/probe-exit"
exit 0
EOF
chmod +x "$tmp/run.sh"

mkdir -p "$PROFILE_DIR"

"$CITRINE_BIN" export iterm2 --palette "$SENTINEL" --out "$tmp/citrine-sentinel.json"
plutil -convert xml1 -o /dev/null "$tmp/citrine-sentinel.json"
plutil -replace Profiles.0.Command -string "$tmp/run.sh" "$tmp/citrine-sentinel.json"
plutil -replace "Profiles.0.Custom Command" -string Yes "$tmp/citrine-sentinel.json"
plutil -convert xml1 -o /dev/null "$tmp/citrine-sentinel.json"
cp "$tmp/citrine-sentinel.json" "$PROFILE_FILE"
cp "$tmp/citrine-sentinel.json" "$OUT_DIR/iterm2-profile.json"

GUID=$(plutil -extract Profiles.0.Guid raw -o - "$PROFILE_FILE")
defaults write com.googlecode.iterm2 SUEnableAutomaticChecks -bool false
defaults write com.googlecode.iterm2 PromptOnQuit -bool false
defaults write com.googlecode.iterm2 NoSyncTipsDisabled -bool true
defaults write com.googlecode.iterm2 "Default Bookmark Guid" -string "$GUID"

open -na iTerm
LAUNCHED=1

if ! wait_for_file "$tmp/result.json" 40; then
  osascript -e 'tell application "iTerm2" to create window with profile "Citrine Sentinel"' > "$OUT_DIR/iterm2-launch.log" 2>&1 &
  OSA_PID=$!
  wait_for_file "$tmp/result.json" 50 || true
  kill "$OSA_PID" >/dev/null 2>&1 || true
fi

if [ "${CI:-}" = "true" ]; then
  pkill -x iTerm2 >/dev/null 2>&1 || true
fi
LAUNCHED=0

if [ ! -s "$tmp/result.json" ]; then
  echo "iterm2 verify: no result produced" >&2
  exit 1
fi

cp "$tmp/result.json" "$OUT_DIR/iterm2-result.json"
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
