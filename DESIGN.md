# Diffusion design

**See what changed.** A quiet workspace for two documents, with no permanent sidebar. The first screen offers two generous file targets and a single compare action. A bundled example makes the real renderer discoverable without pretending sample data are user files.

## Visual language

Warm paper in light mode; ink and slate in dark mode. Muted teal denotes additions, terracotta denotes removals, and periwinkle focuses the current change. Symbols `+`, `−`, and a focus edge reinforce color. Chrome uses proportional typography; source uses a crisp monospace at 14 px with a 24 px line rhythm. Borders are thin, surfaces mostly flat, corners restrained.

The small original mark is two diverging curves. It and the rail express the name through structure, not scientific decoration. No copied assets, colors, icons, or toolbar from another product.

## Comparison

A compact action bar sits above paired file identities. Paths and file details are secondary. Source dominates the window. A 52 px central Diffusion Rail connects changed blocks with smooth ribbons; insertions emerge from a narrow left endpoint and deletions taper to a right endpoint. Stronger inline tints sit within quiet line washes. Blank alignment cells use a subtle rule.

Both panes share vertical movement; horizontal movement preserves unwrapped code. Unchanged regions collapse to a labeled, clickable seam with three context lines retained. Previous/next navigation reveals and centers the current hunk. A thin overview at the right locates changes in the whole comparison.

## Interaction

Drop two files to compare. Drop one to fill the left slot, then another to fill the right. Paste once to fill A with clipboard text and paste again to fill B. Once a pasted comparison is complete, another paste starts a fresh pair at A. File and clipboard inputs may be mixed. Once comparing files, a single drop replaces the side beneath the pointer. Clicking either file target opens a native picker. Errors leave the last successful comparison intact and explain what failed. Loading never replaces the window with a blank screen.

Command on macOS, Control on Linux: O opens files, G / Shift-G navigate, W closes, comma opens preferences. Escape closes preferences. Keyboard focus is visible. Theme offers System / Light / Dark. Differences remain identifiable without red-green discrimination.

No editing or filesystem writes to compared files. No account, network service, telemetry, or roadmap controls that do nothing.
