#!/bin/sh
set -eu
. "$(dirname "$0")/common.sh"
mk_tmp
RESULT="$CITRINE_TMP/result.json"
PROFILE_FILE="$HOME/Library/Application Support/iTerm2/DynamicProfiles/citrine-sentinel.json"
LAUNCHED=0
iterm_cleanup() {
  if [ "$LAUNCHED" -eq 1 ] && [ "${CI:-}" = "true" ]; then
    pkill -x iTerm2 >/dev/null 2>&1 || true
  fi
  rm -f "$PROFILE_FILE"
  citrine_cleanup
}
trap iterm_cleanup EXIT
( sleep 200 && kill -TERM $$ ) >/dev/null 2>&1 &
track_pid $!
"$CITRINE_BIN" verify-setup iterm2 --palette "$SENTINEL" --dir "$CITRINE_TMP" --probe-cmd "$(probe_cmd) --tolerance 16"
mkdir -p verify-out
cp "$PROFILE_FILE" verify-out/iterm2-profile.json
if [ "${CI:-}" = "true" ] && [ -d /Applications/iTerm.app ]; then
  xattr -dr com.apple.quarantine /Applications/iTerm.app >/dev/null 2>&1 || true
fi
echo "iterm2: extracting guid"
GUID=$(sed -n 's/.*"Guid"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$PROFILE_FILE" | head -1)
echo "iterm2: guid=$GUID"
defaults write com.googlecode.iterm2 SUEnableAutomaticChecks -bool false
defaults write com.googlecode.iterm2 PromptOnQuit -bool false
defaults write com.googlecode.iterm2 NoSyncTipsDisabled -bool true
defaults write com.googlecode.iterm2 NoSyncOnboardingWindowHasBeenShown -bool true
defaults write com.googlecode.iterm2 "Default Bookmark Guid" -string "$GUID"
echo "iterm2: defaults written, warm-up launch"
ls -d /Applications/iTerm.app > verify-out/iterm2-open.log 2>&1 || true
open -na /Applications/iTerm.app >> verify-out/iterm2-open.log 2>&1 &
LAUNCHED=1
sleep 20
if [ "${CI:-}" = "true" ]; then
  pkill -x iTerm2 >/dev/null 2>&1 || true
  sleep 5
fi
echo "iterm2: warm-up done, real launch"
open -na /Applications/iTerm.app >> verify-out/iterm2-open.log 2>&1 &
echo "iterm2: open dispatched, waiting for result"
if ! wait_for_file "$RESULT" 40; then
  echo "iterm2: no result yet, trying applescript window"
  osascript -e 'tell application id "com.googlecode.iterm2" to activate' -e 'delay 2' -e 'tell application id "com.googlecode.iterm2" to create window with profile "Citrine Sentinel"' > verify-out/iterm2-launch.log 2>&1 &
  track_pid $!
  wait_for_file "$RESULT" 50 || true
fi
echo "iterm2: wait phase done"
if [ "${CI:-}" = "true" ]; then
  screencapture -x verify-out/iterm2-screen.png >/dev/null 2>&1 || true
fi
if [ "${CI:-}" = "true" ]; then
  pkill -x iTerm2 >/dev/null 2>&1 || true
fi
LAUNCHED=0
cp "$RESULT" verify-out/iterm2-result.json 2>/dev/null || true
finish "$RESULT"
