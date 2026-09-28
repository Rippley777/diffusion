# Validation evidence

2026-09-28. Tests were run locally; CI configuration is supplied but has not been run on a hosted service.

## Verified

| Check | Result |
|---|---|
| macOS Apple Silicon, Rust 1.95.0 | Workspace compiles; optimized native binary builds |
| Formatting / lint | `cargo fmt --all --check`; Clippy across workspace, targets, and features with warnings denied |
| Rust tests on macOS | 13 passing: 8 core tests and 5 UI event/workflow tests |
| Rust tests on Linux ARM64, Debian 12 container | Same workspace tests; no frontend fork |
| macOS Metal render | Real native window opened, comparison loaded asynchronously, light/dark GPU screenshots captured, window closed through the viewport command |
| macOS welcome | Native rendered screenshot inspected; no mockup image |
| Linux X11 render | Xvfb + Mesa software Vulkan, comparison capture and orderly close |
| Linux native X11 drop and navigation | Xdnd source sends original file, then changed file; xdotool sends Control-G. Screenshot shows the second of four hunks selected and scrolled into view |
| Linux native Wayland render | Headless Weston + Mesa software Vulkan, same comparison, capture and orderly close |

The native X11 smoke test uses `scripts/x11-drop.c` as a real Xdnd source. It is test tooling, not an application dependency. Rust UI tests additionally inject paired drops and paste events. They cover filling A and B from consecutive pastes, starting a fresh pair with a third paste, completing a dropped pair when the second file lands on the already occupied welcome target, stale request rejection, error preservation, theme changes, navigation, and close commands.

Core tests cover both document reconstruction for all three algorithms, empty/insert/delete/replace inputs, UTF-8 inline offsets, normalization without source mutation, missing final newline, expansion of folds, a 20,000-line comparison, binary/encoding/size rejection, and multiline syntax state.

Local artifacts (generated, intentionally ignored by version control):

- [macOS dark comparison](../artifacts/macos-comparison.png)
- [macOS light comparison](../artifacts/macos-light.png)
- [macOS welcome](../artifacts/macos-welcome.png)
- [Linux X11 comparison](../artifacts/linux-x11.png)
- [Linux X11 after native drops and Control-G](../artifacts/linux-x11-drop.png)
- [Linux Wayland comparison](../artifacts/linux-wayland.png)
- [Linux test/runtime log](../artifacts/linux-validation.log)

Reproduce the container checks using `scripts/Dockerfile.linux` and `scripts/test-linux.sh`, as described in the README. X11 needs the runtime library `libxkbcommon-x11-0`; the smoke run caught this missing dependency and the documented prerequisites now include it.

## Explicit limitations and remaining manual checks

- **Native Wayland file drop does not work in pinned winit 0.30.13.** Its backend has no drop event implementation; the upstream issue remains open. The app offers file selection there. Use the portal picker, CLI file arguments, or `DIFFUSION_X11=1` under XWayland. Wayland rendering success is not drop-support evidence.
- The XWayland-selection code is compiled on Linux; a physical Wayland desktop with XWayland has not been tested. Native X11 was exercised under Xvfb.
- AppKit and portal picker code is integrated, but interactive picker selection/cancellation has not been automated. Test against the actual portal backend on a normal desktop. The headless container has no portal file picker service.
- Finder file drags, menu interaction, system appearance transitions, clipboard transfer to another app, real mouse-wheel/trackpad scrolling, Retina versus non-Retina displays, and VoiceOver/Orca still need hands-on checks. Basic application event behavior is covered, not all host integration.
- Intel macOS, Linux x86_64 hardware, GPU vendor combinations, GNOME/KDE/Sway, and physical display frame pacing remain unverified. Software-rendered smoke tests make no 60/120 Hz performance claim.
- Full source-document accessibility, arbitrary text selection, and complex-script/font-fallback behavior need additional work. Only visible source rows contribute accessibility labels today.
- No production packaging, signing, notarization, Finder registration, or Linux desktop/MIME associations are claimed.

The result is an operational first two-file slice. These checks do not establish the full premium-product quality bar, and the later roadmap is intentionally unimplemented.
