# Diffusion

**See what changed.** A local, Rust-native two-file comparison app for macOS and Linux.

Diffusion's first slice is deliberately read-only: two documents, quiet line and character highlights, and curved ribbons that connect related changes. No webview, account, telemetry, or network service.

## Run

Install Rust through rustup and the platform prerequisites below. The project pins Rust 1.95.0.

```sh
cargo run --release -p diffusion
cargo run --release -p diffusion -- path/to/original.rs path/to/changed.rs
```

Drop one file, then a second, or drop both together. You can also choose files through the native picker. **Explore an example** opens the bundled sample in the actual comparison engine. A single drop onto an existing comparison replaces the side under the pointer.

- Previous / Next, or Command-G / Shift-Command-G on macOS; Control-G / Shift-Control-G on Linux.
- Click an overview marker to jump to its change.
- Fold unchanged regions; click a seam to expand it.
- Right-click a line to copy the line or that side of its changed block.
- Preferences: System / Light / Dark, font size, Patience / Myers / Histogram, and ignore options.
- Focus hides the main chrome; Escape restores it.
- Command/Control-O opens files, Command/Control-comma opens preferences, Command/Control-W closes.

Compared files are never modified. Appearance preferences are stored by eframe in the local OS application-data directory. Comparison options currently last for the running session.

## Platform prerequisites

**macOS:** Apple command-line developer tools (`xcode-select --install`). Apple Silicon is verified locally; Intel uses the same Rust code but has not been tested. Native application menus, AppKit file dialogs, standard window decorations, Metal rendering, and Retina scaling are enabled.

**Linux:** a Wayland or X11 session, graphics drivers supporting wgpu, and a desktop portal service with a file-picker backend (for example `xdg-desktop-portal-gtk` or the KDE equivalent). Debian/Ubuntu build libraries:

```sh
sudo apt-get install build-essential pkg-config libwayland-dev libxkbcommon-dev \
  libxkbcommon-x11-0 libx11-dev libx11-xcb-dev libxcursor-dev libxi-dev libxrandr-dev libegl1-mesa-dev
```

**Wayland file-drop limitation:** winit 0.30 does not deliver native Wayland drop events. Use the portal picker or CLI there. For file-manager drag/drop on a Wayland desktop with XWayland installed, launch `DIFFUSION_X11=1 cargo run --release -p diffusion`. The welcome screen offers file selection rather than a nonfunctional drop invitation on native Wayland. See the [upstream issue](https://github.com/emilk/egui/issues/1563).

Both display backends are compiled in. Linux uses host window decorations and shared in-window actions. No macOS titlebar imitation. A compositor's appearance reporting may vary; explicit Light and Dark always work.

## Scope and limits

This is a working first vertical slice, not the complete roadmap or a production release.

- UTF-8 text only; binary and UTF-16 input are rejected. Maximum 16 MiB and 250,000 newline characters per file.
- Source rows are virtualized. Syntax highlighting falls back to plain text above 2 MiB or when any line exceeds 8,192 bytes. At most 4,096 characters of each line are displayed; a visible suffix identifies truncation. Copy retains the full line/block.
- Character highlighting is bounded to paired lines of at most 4,096 bytes, with a total inline time budget. Huge replacements retain line highlighting. Line diff has a two-second deadline that can yield coarser hunks.
- Changed lines pair by position, not semantic similarity. No moved-block detection. Tabs display as four spaces. A `¬` suffix marks a missing terminal newline.
- AccessKit is enabled and visible source rows expose labels, but full-document screen-reader navigation and arbitrary text-range selection are not finished. Copy line/block is available.
- No editing, merge, folder comparison, Git, file watching, search, unified view, or review sessions yet. No placeholder buttons for these features.
- Native file pickers are modal; analysis runs in a bounded background worker. New requests replace queued work and stale results cannot overwrite the current comparison. An in-progress algorithm runs until its time limit; cancellation is cooperative between stages.

See [ARCHITECTURE.md](ARCHITECTURE.md), [DESIGN.md](DESIGN.md), and [validation evidence](docs/VALIDATION.md).

## Development

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Core tests cover reconstruction, newline differences, Unicode inline boundaries, normalization, collapse, and 20,000-line input. UI event tests cover the sequential/paired-drop workflow, navigation, themes, close, error preservation, and stale requests.

Optional native rendering captures (no desktop screen-recording permission required):

```sh
mkdir -p artifacts
cargo build -p diffusion --features screenshot
DIFFUSION_CAPTURE=artifacts/comparison.png DIFFUSION_CAPTURE_THEME=dark \
  target/debug/diffusion fixtures/before.rs fixtures/after.rs
```

The optional screenshot feature captures the app's own GPU surface and exits. Without that feature, capture environment variables do nothing.

Linux container validation:

```sh
docker build -t diffusion-linux-test -f scripts/Dockerfile.linux .
docker run --rm -v "$PWD:/work" -v diffusion-linux-target:/build \
  -v diffusion-linux-cargo:/usr/local/cargo/registry diffusion-linux-test \
  sh scripts/test-linux.sh
```
