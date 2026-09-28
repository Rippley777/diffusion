#!/bin/sh
set -eu
mkdir -p artifacts
cargo test --workspace --all-features --locked
cargo build -p diffusion --features screenshot --locked
export XDG_RUNTIME_DIR="$(mktemp -d /tmp/diffusion-runtime.XXXXXX)"
chmod 700 "$XDG_RUNTIME_DIR"
export WGPU_BACKEND=vulkan
trap 'if [ -n "${weston_pid:-}" ]; then kill "$weston_pid" 2>/dev/null || true; fi; rm -rf "$XDG_RUNTIME_DIR"' EXIT
DIFFUSION_CAPTURE=/work/artifacts/linux-x11.png DIFFUSION_CAPTURE_THEME=light \
  timeout 45s xvfb-run -a /build/debug/diffusion fixtures/before.rs fixtures/after.rs
cc scripts/x11-drop.c -o /build/x11-drop -lX11
timeout 45s xvfb-run -a sh -eu -c '
  DIFFUSION_CAPTURE=/work/artifacts/linux-x11-drop.png DIFFUSION_CAPTURE_WAIT=1 DIFFUSION_CAPTURE_FRAMES=120 /build/debug/diffusion &
  app_pid=$!
  trap "kill $app_pid 2>/dev/null || true" EXIT
  /build/x11-drop fixtures/before.rs
  /build/x11-drop fixtures/after.rs
  sleep 0.6
  window=$(xdotool search --name '^Diffusion$' | head -1)
  xdotool windowfocus --sync "$window" key ctrl+g
  wait "$app_pid"
'
weston --backend=headless-backend.so --use-pixman --socket=diffusion-wayland \
  --idle-time=0 --width=1440 --height=1000 > artifacts/weston.log 2>&1 &
weston_pid=$!
count=0
while [ ! -S "$XDG_RUNTIME_DIR/diffusion-wayland" ]; do
  count=$((count + 1))
  if [ "$count" -gt 50 ]; then cat artifacts/weston.log; exit 1; fi
  sleep 0.1
done
WAYLAND_DISPLAY=diffusion-wayland DIFFUSION_CAPTURE=/work/artifacts/linux-wayland.png \
  DIFFUSION_CAPTURE_THEME=dark timeout 45s /build/debug/diffusion fixtures/before.rs fixtures/after.rs
test -s artifacts/linux-x11.png
test -s artifacts/linux-wayland.png
