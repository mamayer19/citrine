# Citrine

Design terminal color schemes in your terminal. Citrine is a fast TUI where you tune a palette, watch it repaint your real terminal live, and export it straight to your terminal's config.

![Citrine demo](demo.gif)

![CI](https://github.com/mamayer19/citrine/actions/workflows/ci.yml/badge.svg)

## Features

- **Live in your real terminal.** Turn on live mode and Citrine repaints the terminal you are sitting in through OSC escape codes, so changing a color instantly re-renders your prompt, your output, and everything on screen. Quit and your theme is restored.
- **A true preview.** Even with live mode off, a preview pane renders a shell prompt, a colored file listing, a code snippet, and the full ANSI palette in truecolor as you edit.
- **Readability built in.** A contrast panel shows the live WCAG ratio with AA and AAA marks for the color you are editing, so you never ship an unreadable theme.
- **Edit the way you think.** Nudge HSL or OKLCH channels or type a hex value, and start from bundled reference palettes like Rose Pine, Catppuccin, and Gruvbox.
- **Save and apply anywhere.** Keep a local library of palettes and apply a finished one straight to your terminal's config, or to a custom path you choose.

## Supported terminals

Ghostty, Kitty, Alacritty, WezTerm, iTerm2 (as an auto-loaded Dynamic Profile), foot, rio, and konsole.

## Install

Prebuilt binaries are coming. For now, build from source with [rustup](https://rustup.rs):

```sh
git clone https://github.com/mamayer19/citrine
cd citrine
cargo build --release -p citrine
```

That produces a single self-contained binary at `target/release/citrine`. Copy it somewhere on your `PATH`, such as `~/.local/bin/`, to run `citrine` from anywhere.

## Usage

Run `citrine` to open the editor. It imports your current Ghostty theme if it finds one, otherwise it opens on the default palette, Citrus Field (Dawn).

| Key | Action |
| --- | --- |
| `up` `down` or `j` `k` | move the slot selection |
| `left` `right` or `Tab` | cycle the active channel |
| `-` `+` (`[` `]`) | adjust the channel (`{` `}` for a bigger step) |
| `x` | switch color model (HSL or OKLCH) |
| `e` | type a hex value |
| `space` | toggle live-apply to your real terminal |
| `r` | cycle reference palettes |
| `i` | re-import your current Ghostty theme |
| `u` | undo |
| `s` | apply the palette to a terminal |
| `w` | save the palette to your library |
| `o` | open the library |
| `p` | set a custom target path for a terminal |
| `?` | help |
| `q` | quit |

## Contributing

Citrine is two crates: `citrine-core` (the color math and terminal formats) and `citrine` (the TUI). The `./dev` helper wraps the common tasks: `./dev check` formats, lints, and tests; `./dev coverage` prints a coverage summary; `./dev fuzz-list` shows the fuzz targets. CI runs the tests on Linux, macOS, and Windows.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
