# Diffusion architecture

Decision date: 2026-09-28. Repository inspection found an empty directory, without an existing application or build constraints.

## Decision

Use **egui/eframe 0.36 with wgpu**, a platform-independent Rust domain crate, and a small desktop integration module. No browser, JavaScript runtime, or webview. Ship one shared frontend for macOS and Linux. This is a reversible choice: document and diff models contain no GUI types.

This recommendation weighs the first read-only, two-file slice. It is not a claim that egui has better text shaping than GPUI or Iced. Font fallback, bidirectional source presentation, accessibility, and native integration remain explicit acceptance gates before calling the product production-ready.

## Current ecosystem comparison

Research uses upstream documentation and manifests, not popularity rankings. “Supported” means upstream capability, not verification on every compositor or machine.

| Criterion | GPUI | Iced | egui / eframe (selected) |
|---|---|---|---|
| macOS | Metal; editor-oriented platform/text integration | wgpu Metal; custom widgets | wgpu Metal; native winit window, custom widgets |
| Linux | Wayland and X11 platform backends | Wayland and X11 features | Wayland and X11 features enabled explicitly |
| Text | Strong editor pedigree; platform text backend | cosmic-text shaping; strongest candidate for complex scripts | HarfRust shaping and skrifa glyphs in 0.36; cached layout, bundled fonts, Retina scaling |
| Large documents | Uniform lists/custom elements | Requires deliberate virtualized custom widget | Visible-row rendering with scroll viewport; layout only visible lines |
| GPU / rail | Custom GPU drawing | Canvas / custom widgets | Painter meshes and curves; straightforward clipping |
| Accessibility | Upstream accessibility work; verify selected release | Do not assume a complete production screen-reader path | AccessKit integration; painted text needs explicit semantics |
| File drop / keys | Platform events and actions | Window events and subscriptions | macOS/X11 drops; native Wayland drop unavailable in winit 0.30; command/control modifiers |
| Menus on macOS | Built-in menu API | Additional native integration | Additional native integration, isolated in platform module |
| Linux menus | Host conventions require testing | In-window UI is portable | In-window controls; no imitation macOS titlebar |
| Clipboard | Platform services | Runtime clipboard commands | egui-winit integration, including Wayland |
| File dialogs | Platform prompt APIs | External rfd integration | rfd: AppKit on macOS, XDG portal on Linux |
| Theme / HiDPI | Platform support | Linux theme feature, scale-aware rendering | System theme and scale from windowing integration |
| Filesystem watching | Independent Rust service | Independent Rust service | Independent Rust service; not implemented in this slice |
| Maintenance | Pre-1.0, closely tied to Zed; integration churn | Active, experimental, strong message/task architecture | Active modular project; breaking upgrades require pinning and checks |

GPUI is a serious runner-up, especially if a full editable text surface becomes necessary. Its current README describes separate platform features and warns of frequent breaking changes. Iced has a clean task model and advanced shaping, but Diffusion would still need a purpose-built virtualized comparison widget. Eframe makes the smallest auditable custom-rendered slice possible; retaining that advantage depends on keeping UI state separate from analysis.

Upstream sources consulted:

- [GPUI overview and examples](https://gpui.rs/), [platform requirements and development status](https://raw.githubusercontent.com/zed-industries/zed/main/crates/gpui/README.md).
- [Iced architecture and renderers](https://github.com/iced-rs/iced), [current feature and text dependencies](https://raw.githubusercontent.com/iced-rs/iced/master/Cargo.toml). The latter is development HEAD, not a released-version guarantee.
- [eframe feature flags](https://docs.rs/eframe/0.36.2/eframe/), [egui architecture](https://github.com/emilk/egui/blob/main/ARCHITECTURE.md), [virtualized scroll API](https://docs.rs/egui/latest/egui/containers/scroll_area/struct.ScrollArea.html).
- [Native dialogs and portal prerequisites](https://github.com/PolyMeilex/rfd), [notify backends](https://github.com/notify-rs/notify).

## Engine and data flow

`diffusion-core` owns UTF-8 document loading, line ranges, options, diff hunks, inline byte ranges, collapsed-row projection, and syntax spans. `diffusion` owns interaction, worker scheduling, and rendering. One background analysis worker accepts the newest pending request; generation checks discard stale results. Reading, line diffing, and syntax work happen outside the UI thread. Diff algorithms have time limits, and oversized/binary input produces an actionable error instead of silently corrupting text.

`Document → DiffEngine + DiffOptions → DiffResult { DiffHunk, DiffLine, InlineChange } → visible rows → UI`

`history.rs` owns serializable text snapshots, deduplication, pinned versions, and bounded retention independently of UI rendering. Completed clipboard/scratchpad requests record the successful result, guarded by the worker generation. A scratchpad edit updates its unpinned history entry; pinned entries branch on changes. Reopening loads content into in-memory inputs instead of re-reading paths. History and the current scratchpad draft use versioned eframe storage keys, with five-second autosaves and normal-exit saving. The editor compares on demand, so syntax highlighting still runs in the existing worker rather than on every keystroke.

Rows align both sides in one vertical scroll surface, eliminating synchronization drift. Hunks retain real source ranges; blank cells are alignment space, never fabricated source lines. Inline ranges use UTF-8 byte boundaries. The rail derives geometry from those ranges. Collapsing is a projection, not destructive modification of the result.

## Diff algorithms

[similar 3.2](https://docs.rs/similar/3.2.0/similar/enum.Algorithm.html) provides Myers, Patience, and Histogram. Default to **Patience** for readable source anchors. Myers is useful for short or repetitive inputs; Histogram broadens anchor selection to low-frequency lines. Expose all three rather than inventing an LCS implementation. Use Myers for bounded character comparisons of paired changed lines. Ordinal line pairing is deliberately simple and can misalign rewritten blocks; moved-block detection is deferred. Deadlines may produce a coarser valid diff.

## Syntax

Use [syntect](https://github.com/trishume/syntect) for this read-only slice: broad bundled languages and stateful multiline highlighting, computed by the worker. [Tree-sitter](https://tree-sitter.github.io/tree-sitter/3-syntax-highlighting.html) is preferable when incremental editing or semantic comparison becomes real work. Its per-language grammars and queries add distribution/maintenance work today. Syntax spans remain independent of UI types so that provider can change without rewriting the renderer. Long lines and large documents fall back to plain text within documented limits.

## Platform boundaries and acceptance

- Keep file dialogs, native menus, and window behavior in `platform.rs`. macOS uses real OS decorations; Linux uses the compositor/window manager. No simulated traffic lights. No mandatory translucency.
- Linux requires a working GPU backend and desktop portal for file dialogs. Native menus are a macOS enhancement; shared toolbar actions are the Linux fallback.
- No future Git, merge, image, review, or watcher stubs. `notify` is a suitable later service (FSEvents/inotify), not a dependency until live updates are implemented.
- Build and core tests on both platforms, then exercise launch, sequential drop, paired drop, navigation, collapse, appearance, and close. Headless Linux runs complement, but cannot replace, a real compositor/file-manager test. Record observed results in `docs/VALIDATION.md` without equating compilation with UX verification.
- Font coverage, bidirectional text, assistive reading of virtualized source text, and physical Retina/scroll performance need manual validation. No unmeasured startup, memory, or frame-rate promises.

Text implementation was also checked against the downloaded epaint 0.36.2 source and its [published dependencies](https://docs.rs/crate/epaint/0.36.2): older statements about egui lacking shaping no longer describe this release. A shaping library alone does not establish correct bidirectional diff presentation or complete font coverage.

## Linux integration finding

Native X11 and Wayland rendering both passed headless runtime checks. Inspection of pinned winit 0.30.13 and the [open upstream drop issue](https://github.com/emilk/egui/issues/1563) identified a real limitation: native Wayland file drops are unavailable. `platform.rs` isolates the capability check and optional `DIFFUSION_X11=1` event-loop selection. Native Wayland uses the portal picker/CLI, and the welcome screen does not promise drop support. XWayland is an optional file-drop fallback; native Wayland rendering remains enabled. This is an accepted first-slice exception under the Linux MVP rule, not completed Wayland feature parity. Resolving it requires a newer winit backend, an upstream contribution, or revisiting GPUI; do not invent a second Wayland connection for the same window.
