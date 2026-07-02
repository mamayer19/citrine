#!/bin/sh
set -eu
. "$(dirname "$0")/common.sh"
mk_tmp
RESULT="$CITRINE_TMP/result.json"
mkdir -p "$CITRINE_TMP/colors"
"$CITRINE_BIN" export wezterm --palette "$SENTINEL" --out "$CITRINE_TMP/colors/Citrine Sentinel.toml"
PROBE=$(probe_cmd)
cat > "$CITRINE_TMP/wezterm.lua" <<EOF
return {
  color_scheme_dirs = { "$CITRINE_TMP/colors" },
  color_scheme = "Citrine Sentinel",
  default_prog = { "/bin/sh", "-c", [[$PROBE]] },
  enable_wayland = false,
  front_end = "Software",
}
EOF
wezterm --config-file "$CITRINE_TMP/wezterm.lua" ls-fonts > /dev/null
wezterm --config-file "$CITRINE_TMP/wezterm.lua" start > "$CITRINE_TMP/wezterm.log" 2>&1 &
track_pid $!
wait_for_file "$RESULT" 60 || true
mkdir -p verify-out
cp "$CITRINE_TMP/wezterm.log" verify-out/wezterm.log 2>/dev/null || true
finish "$RESULT"
