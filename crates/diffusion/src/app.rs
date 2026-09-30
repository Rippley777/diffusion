use crate::{
    history::{History, Scratchpad},
    platform,
    theme::Palette,
    worker::{Comparison, Input, Request, Worker},
};
use diffusion_core::{
    ChangeType, DiffOptions, DisplayRow, Document, InlineChange, MAX_FILE_BYTES, Strategy, Syntax,
};
use eframe::egui::{
    self, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, pos2, vec2,
};
use std::{collections::HashSet, path::PathBuf};

const ROW: f32 = 24.0;
const RAIL: f32 = 52.0;
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct Preferences {
    theme: u8,
    collapse: bool,
    font_size: f32,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: 0,
            collapse: true,
            font_size: 14.0,
        }
    }
}

pub struct Diffusion {
    inputs: [Option<Input>; 2],
    comparison: Option<Comparison>,
    worker: Worker,
    generation: u64,
    loading: bool,
    error: Option<String>,
    prefs: Preferences,
    options: DiffOptions,
    settings: bool,
    current: usize,
    expanded: HashSet<usize>,
    display: Vec<DisplayRow>,
    hunk_positions: Vec<usize>,
    jump: bool,
    horizontal: f32,
    demo: bool,
    focus: bool,
    menu: platform::NativeMenu,
    drop_targets: [Rect; 2],
    history: History,
    history_open: bool,
    history_query: String,
    history_id: Option<u64>,
    history_notice: Option<String>,
    scratchpad: Scratchpad,
    editor_open: bool,
    editor_origin: Option<u64>,
    editor_error: Option<String>,
    #[cfg(feature = "screenshot")]
    capture_frames: u32,
}
impl Diffusion {
    pub fn new(cc: &eframe::CreationContext<'_>, paths: Vec<PathBuf>) -> Self {
        let prefs = cc
            .storage
            .and_then(|s| eframe::get_value(s, "preferences"))
            .unwrap_or_default();
        let mut app = Self {
            inputs: [None, None],
            comparison: None,
            worker: Worker::new(cc.egui_ctx.clone()),
            generation: 0,
            loading: false,
            error: None,
            prefs,
            options: DiffOptions::default(),
            settings: false,
            current: 0,
            expanded: HashSet::new(),
            display: vec![],
            hunk_positions: vec![],
            jump: false,
            horizontal: 0.0,
            demo: false,
            focus: false,
            menu: platform::NativeMenu::new(&cc.egui_ctx),
            drop_targets: [Rect::NOTHING; 2],
            history: cc
                .storage
                .and_then(|s| eframe::get_value(s, "comparison_history_v1"))
                .unwrap_or_default(),
            history_open: false,
            history_query: String::new(),
            history_id: None,
            history_notice: None,
            scratchpad: cc
                .storage
                .and_then(|s| eframe::get_value(s, "scratchpad_v1"))
                .unwrap_or_else(Scratchpad::blank),
            editor_open: false,
            editor_origin: None,
            editor_error: None,
            #[cfg(feature = "screenshot")]
            capture_frames: 0,
        };
        #[cfg(feature = "screenshot")]
        if let Ok(theme) = std::env::var("DIFFUSION_CAPTURE_THEME") {
            app.prefs.theme = if theme == "light" { 1 } else { 2 };
        }
        app.accept_paths(paths, None);
        app
    }
    fn accept_paths(&mut self, paths: Vec<PathBuf>, side: Option<usize>) {
        if paths.is_empty() {
            return;
        }
        if paths.len() > 2 {
            self.error = Some("Drop one or two files at a time.".into());
            return;
        }
        if paths.iter().any(|p| !p.is_file()) {
            self.error = Some("Choose regular text files. Folder comparison comes later.".into());
            return;
        }
        if self.demo {
            self.inputs = [None, None];
            self.comparison = None;
            self.hunk_positions.clear();
        }
        self.demo = false;
        self.history_id = None;
        if paths.len() == 2 {
            self.inputs = [
                Some(Input::File(paths[0].clone())),
                Some(Input::File(paths[1].clone())),
            ];
        } else {
            let index = side.unwrap_or(if self.inputs[0].is_none() { 0 } else { 1 });
            self.inputs[index] = Some(Input::File(paths[0].clone()));
        }
        self.request();
    }
    fn accept_clipboard(&mut self, text: String) {
        if text.is_empty() {
            self.error = Some("The clipboard does not contain text.".into());
            return;
        }
        if text.len() as u64 > MAX_FILE_BYTES
            || text.bytes().filter(|b| *b == b'\n').count() > 250_000
        {
            self.error = Some("Clipboard text exceeds the 16 MiB or 250,000-line limit.".into());
            return;
        }
        if self.demo || self.inputs.iter().all(Option::is_some) {
            self.new_comparison();
        }
        let side = self.inputs.iter().position(Option::is_none).unwrap_or(0);
        self.inputs[side] = Some(Input::Clipboard {
            name: format!("Clipboard {}", if side == 0 { "A" } else { "B" }),
            text,
        });
        self.error = None;
        self.request();
    }
    fn new_comparison(&mut self) {
        self.generation += 1;
        self.loading = false;
        self.comparison = None;
        self.inputs = [None, None];
        self.display.clear();
        self.hunk_positions.clear();
        self.current = 0;
        self.demo = false;
        self.history_id = None;
        self.error = None;
        self.editor_open = false;
    }
    fn request(&mut self) {
        if let [Some(a), Some(b)] = &self.inputs {
            self.generation += 1;
            self.loading = true;
            self.error = None;
            self.worker.submit(Request {
                generation: self.generation,
                inputs: [a.clone(), b.clone()],
                options: self.options.clone(),
                demo: self.demo,
            });
        }
    }
    fn example(&mut self) {
        self.inputs = [
            Some(Input::File("example/before.rs".into())),
            Some(Input::File("example/after.rs".into())),
        ];
        self.demo = true;
        self.request();
    }
    fn pick(&mut self, side: Option<usize>) {
        if let Some(paths) = platform::pick_files() {
            self.accept_paths(paths, side);
        }
    }
    fn project(&mut self) {
        if let Some(c) = &self.comparison {
            self.display = c.diff.display_rows(self.prefs.collapse, &self.expanded);
            self.hunk_positions = vec![0; c.diff.hunks.len()];
            for (position, row) in self.display.iter().enumerate() {
                if let DisplayRow::Line(index) = row
                    && let Some(h) = c.diff.rows[*index].hunk
                    && c.diff.hunks[h].rows.start == *index
                {
                    self.hunk_positions[h] = position;
                }
            }
        }
    }
    fn navigate(&mut self, forward: bool) {
        let n = self.hunk_positions.len();
        if n > 0 {
            self.current = if forward {
                (self.current + 1) % n
            } else {
                (self.current + n - 1) % n
            };
            self.jump = true;
        }
    }
    fn shortcuts(&mut self, ctx: &egui::Context) {
        let command = egui::Modifiers::COMMAND;
        let pasted = ctx.input(|i| {
            i.events.iter().rev().find_map(|event| match event {
                egui::Event::Paste(text) => Some(text.clone()),
                _ => None,
            })
        });
        if let Some(text) = pasted
            && !self.editor_open
            && !self.history_open
            && !ctx.egui_wants_keyboard_input()
        {
            self.accept_clipboard(text);
        }
        if ctx.input_mut(|i| i.consume_key(command, egui::Key::O)) {
            self.pick(None);
        }
        if ctx.input_mut(|i| i.consume_key(command | egui::Modifiers::SHIFT, egui::Key::N)) {
            self.scratchpad = Scratchpad::blank();
            self.open_editor(false);
        } else if ctx.input_mut(|i| i.consume_key(command, egui::Key::N)) {
            self.new_comparison();
        }
        if ctx.input_mut(|i| i.consume_key(command, egui::Key::E)) {
            self.open_editor(true);
        }
        if ctx.input_mut(|i| i.consume_key(command | egui::Modifiers::SHIFT, egui::Key::H)) {
            self.history_open = !self.history_open;
        }
        if ctx.input_mut(|i| i.consume_key(command | egui::Modifiers::SHIFT, egui::Key::G)) {
            self.navigate(false);
        } else if ctx.input_mut(|i| i.consume_key(command, egui::Key::G)) {
            self.navigate(true);
        }
        if ctx.input_mut(|i| i.consume_key(command, egui::Key::W)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ctx.input_mut(|i| i.consume_key(command, egui::Key::Comma)) {
            self.settings = !self.settings;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.settings = false;
            self.focus = false;
            self.editor_open = false;
            self.history_open = false;
        }
    }
    fn toolbar(&mut self, ui: &mut egui::Ui, p: Palette) {
        let response = egui::Frame::new()
            .fill(p.surface)
            .inner_margin(egui::Margin::symmetric(20, 10))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(24.0, 26.0), Sense::hover());
                    mark(ui.painter(), r.center(), 12.0, p.removed, p.added);
                    ui.label(RichText::new("Diffusion").size(17.0).strong());
                    ui.add_space(16.0);
                    if ui
                        .button("Open files…")
                        .on_hover_text(platform::shortcut("O"))
                        .clicked()
                    {
                        self.pick(None);
                    }
                    if self.comparison.is_some()
                        && ui
                            .button("New")
                            .on_hover_text(platform::shortcut("N"))
                            .clicked()
                    {
                        self.new_comparison();
                    }
                    if self.loading {
                        ui.spinner();
                        ui.label(RichText::new("Comparing…").color(p.muted));
                    }
                    if ui
                        .button(if self.comparison.is_some() {
                            "Edit text"
                        } else {
                            "Scratchpad"
                        })
                        .on_hover_text(format!(
                            "{} · Edit either side; source files are never changed",
                            platform::shortcut("E")
                        ))
                        .clicked()
                    {
                        self.open_editor(self.comparison.is_some());
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button("Preferences")
                            .on_hover_text(platform::shortcut(","))
                            .clicked()
                        {
                            self.settings = !self.settings;
                        }
                        if ui
                            .button("History")
                            .on_hover_text(platform::shortcut("Shift+H"))
                            .clicked()
                        {
                            self.history_open = !self.history_open;
                        }
                        if self.comparison.is_some() && !self.demo && !self.loading {
                            let pinned = self.history_id.is_some_and(|id| {
                                self.history.entries.iter().any(|e| e.id == id && e.pinned)
                            });
                            if ui
                                .selectable_label(pinned, if pinned { "Pinned" } else { "Pin" })
                                .clicked()
                            {
                                self.pin_current();
                            }
                        }
                        if self.comparison.is_some()
                            && ui.selectable_label(self.focus, "Focus").clicked()
                        {
                            self.focus = !self.focus;
                        }
                    });
                });
            });
        spectrum_line(
            ui.painter(),
            response.response.rect.left_bottom(),
            response.response.rect.right_bottom(),
            p.removed,
            p.modified,
            p.added,
        );
    }
    fn welcome(&mut self, ui: &mut egui::Ui, p: Palette) {
        let compact = ui.available_height() < 620.0;
        ui.add_space(if compact {
            14.0
        } else {
            (ui.available_height() * 0.17).max(24.0)
        });
        ui.vertical_centered(|ui| {
            let (r, _) = ui.allocate_exact_size(
                egui::Vec2::splat(if compact { 38.0 } else { 64.0 }),
                Sense::hover(),
            );
            mark(
                ui.painter(),
                r.center(),
                if compact { 18.0 } else { 28.0 },
                p.removed,
                p.added,
            );
            ui.add_space(if compact { 10.0 } else { 22.0 });
            ui.label(
                RichText::new("See what changed.")
                    .size(if compact { 28.0 } else { 34.0 })
                    .color(p.text),
            );
            ui.add_space(10.0);
            ui.label(
                RichText::new("Two files. A clearer perspective.")
                    .size(16.0)
                    .color(p.muted),
            );
            ui.add_space(if compact { 12.0 } else { 38.0 });
            ui.scope(|ui| {
                ui.set_max_width(620.0);
                ui.columns(2, |cols| {
                    for (i, col) in cols.iter_mut().enumerate() {
                        let loaded = self.inputs[i].as_ref();
                        let title = loaded
                            .map(|input| match input {
                                Input::File(path) => path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .into_owned(),
                                Input::Clipboard { name, .. } | Input::Scratchpad { name, .. } => {
                                    name.clone()
                                }
                            })
                            .unwrap_or_else(|| {
                                if i == 0 {
                                    "Original file".into()
                                } else {
                                    "Changed file".into()
                                }
                            });
                        let text = format!(
                            "{}\n\n{}\n\n{}",
                            if i == 0 { "A" } else { "B" },
                            title,
                            if loaded.is_some() {
                                "Click to replace"
                            } else if platform::file_drop_available() {
                                "Drop, choose, or paste text"
                            } else {
                                "Choose a file"
                            }
                        );
                        let side_color = if i == 0 { p.removed } else { p.added };
                        let side_fill = if i == 0 { p.remove_wash } else { p.add_wash };
                        let response = col.add_sized(
                            [col.available_width(), if compact { 120.0 } else { 145.0 }],
                            egui::Button::new(RichText::new(text).size(15.0))
                                .fill(side_fill.gamma_multiply(0.72))
                                .stroke(Stroke::new(1.2, side_color.gamma_multiply(0.7)))
                                .corner_radius(8),
                        );
                        self.drop_targets[i] = response.rect;
                        if response.clicked() {
                            self.pick(Some(i));
                        }
                    }
                });
            });
            ui.add_space(if compact { 10.0 } else { 24.0 });
            if ui.button("Explore an example").clicked() {
                self.example();
            }
            ui.label(
                RichText::new(format!(
                    "Paste text with {} twice to compare clipboard contents",
                    platform::shortcut("V")
                ))
                .size(11.0)
                .color(p.muted),
            );
            ui.add_space(if compact { 12.0 } else { 38.0 });
            ui.label(
                RichText::new("LOCAL BY DESIGN  ·  YOUR FILES STAY YOURS")
                    .size(10.0)
                    .color(p.muted),
            );
        });
    }
    fn file_headers(&mut self, ui: &mut egui::Ui, p: Palette) {
        let Some(c) = &self.comparison else {
            return;
        };
        let mut replace = None;
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(20, 13))
            .show(ui, |ui| {
                ui.columns(2, |cols| {
                    for (side, col) in cols.iter_mut().enumerate() {
                        let (doc, syntax) = if side == 0 {
                            (&c.left, &c.left_syntax)
                        } else {
                            (&c.right, &c.right_syntax)
                        };
                        col.horizontal(|ui| {
                            ui.label(
                                RichText::new(if side == 0 { "A" } else { "B" })
                                    .color(if side == 0 { p.removed } else { p.added })
                                    .strong(),
                            );
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(doc.name()).size(15.0).strong(),
                                    )
                                    .frame(false),
                                )
                                .on_hover_text(format!(
                                    "{}\n{} bytes · UTF-8 · {} · {}\nClick to replace",
                                    doc.path.display(),
                                    doc.text.len(),
                                    doc.line_ending,
                                    syntax.language
                                ))
                                .clicked()
                            {
                                replace = Some(side);
                            }
                            ui.label(
                                RichText::new(format!("{} lines", doc.lines.len()))
                                    .size(11.0)
                                    .color(p.muted),
                            );
                        });
                        col.label(
                            RichText::new(
                                doc.path
                                    .parent()
                                    .unwrap_or_else(|| std::path::Path::new(""))
                                    .display()
                                    .to_string(),
                            )
                            .size(11.0)
                            .color(p.muted),
                        );
                    }
                });
            });
        if let Some(side) = replace {
            self.pick(Some(side));
        }
    }
    fn status(&mut self, ui: &mut egui::Ui, p: Palette) {
        egui::Frame::new()
            .fill(p.surface)
            .inner_margin(egui::Margin::symmetric(20, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let count = self.hunk_positions.len();
                    if ui
                        .add_enabled(count > 0, egui::Button::new("Previous"))
                        .on_hover_text(format!(
                            "Previous change · Shift+{}",
                            platform::shortcut("G")
                        ))
                        .clicked()
                    {
                        self.navigate(false);
                    }
                    ui.label(
                        RichText::new(if count == 0 {
                            "No differences".into()
                        } else {
                            format!("{} / {} changes", self.current + 1, count)
                        })
                        .size(12.0),
                    );
                    if ui
                        .add_enabled(count > 0, egui::Button::new("Next"))
                        .on_hover_text(format!("Next change · {}", platform::shortcut("G")))
                        .clicked()
                    {
                        self.navigate(true);
                    }
                    ui.add_space(12.0);
                    if let Some(c) = &self.comparison {
                        ui.label(
                            RichText::new(format!("+{}", c.diff.additions))
                                .color(p.added)
                                .monospace(),
                        );
                        ui.label(
                            RichText::new(format!("−{}", c.diff.removals))
                                .color(p.removed)
                                .monospace(),
                        );
                        if c.left_syntax.limited || c.right_syntax.limited {
                            ui.label(
                                RichText::new("Plain text · large input")
                                    .size(11.0)
                                    .color(p.muted),
                            );
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .checkbox(&mut self.prefs.collapse, "Fold unchanged")
                            .changed()
                        {
                            self.project();
                        }
                        if self.demo {
                            ui.label(RichText::new("EXAMPLE").size(10.0).color(p.muted));
                        }
                    });
                });
            });
    }
    fn preferences(&mut self, ctx: &egui::Context) {
        let mut open = self.settings;
        let mut recompute = false;
        egui::Window::new("Preferences")
            .open(&mut open)
            .resizable(false)
            .default_width(330.0)
            .show(ctx, |ui| {
                ui.label(RichText::new("Appearance").strong());
                ui.horizontal(|ui| {
                    for (i, name) in ["System", "Light", "Dark"].iter().enumerate() {
                        ui.selectable_value(&mut self.prefs.theme, i as u8, *name);
                    }
                });
                ui.add(egui::Slider::new(&mut self.prefs.font_size, 11.0..=18.0).text("Code size"));
                ui.add_space(12.0);
                ui.label(RichText::new("Comparison").strong());
                egui::ComboBox::from_label("Algorithm")
                    .selected_text(format!("{:?}", self.options.strategy))
                    .show_ui(ui, |ui| {
                        for strategy in [Strategy::Patience, Strategy::Myers, Strategy::Histogram] {
                            recompute |= ui
                                .selectable_value(
                                    &mut self.options.strategy,
                                    strategy,
                                    format!("{strategy:?}"),
                                )
                                .changed();
                        }
                    });
                recompute |= ui
                    .checkbox(
                        &mut self.options.ignore_whitespace,
                        "Ignore spaces and tabs",
                    )
                    .changed();
                recompute |= ui
                    .checkbox(&mut self.options.ignore_case, "Ignore case")
                    .changed();
                recompute |= ui
                    .checkbox(
                        &mut self.options.ignore_line_endings,
                        "Ignore CRLF / LF differences",
                    )
                    .changed();
                ui.add_space(12.0);
                ui.label(
                    RichText::new("Files are read-only. Preferences stay on this device.").small(),
                );
            });
        self.settings = open;
        if recompute {
            self.request();
        }
    }
    fn comparison_view(&mut self, ui: &mut egui::Ui, p: Palette) {
        let Some(c) = &self.comparison else {
            return;
        };
        let dark = ui.visuals().dark_mode;
        let width = ui.available_width();
        let overview_width = 14.0;
        let content_width = width - overview_width;
        let pane = (content_width - RAIL) / 2.0;
        let max_horizontal =
            (c.max_columns as f32 * self.prefs.font_size * 0.65 - pane + 90.0).max(0.0);
        if ui.rect_contains_pointer(ui.max_rect()) {
            self.horizontal = (self.horizontal - ui.input(|i| i.smooth_scroll_delta.x))
                .clamp(0.0, max_horizontal);
        }
        let mut expanded = None;
        let mut clicked_hunk = None;
        let height = ui.available_height() - 26.0;
        let mut scroll = egui::ScrollArea::vertical()
            .id_salt("comparison")
            .auto_shrink([false, false])
            .max_height(height);
        if self.jump {
            if let Some(row) = self.hunk_positions.get(self.current) {
                scroll = scroll.vertical_scroll_offset((*row as f32 * ROW - height * 0.3).max(0.0));
            }
            self.jump = false;
        }
        ui.spacing_mut().item_spacing.y = 0.0;
        let mut view_rect = Rect::NOTHING;
        scroll.show_viewport(ui, |ui, viewport| {
            let (full, _) = ui.allocate_exact_size(
                vec2(content_width, self.display.len().max(1) as f32 * ROW + 24.0),
                Sense::hover(),
            );
            view_rect = ui.clip_rect();
            let first = (viewport.min.y / ROW).floor().max(0.0) as usize;
            let last = ((viewport.max.y / ROW).ceil() as usize + 1).min(self.display.len());
            let rail_x = full.left() + pane;
            ui.painter().rect_filled(
                Rect::from_min_max(pos2(rail_x, full.top()), pos2(rail_x + RAIL, full.bottom())),
                0,
                p.surface,
            );
            let mut visible_hunks = HashSet::new();
            for index in first..last {
                let y = full.top() + index as f32 * ROW;
                let rect = Rect::from_min_size(pos2(full.left(), y), vec2(content_width, ROW));
                match &self.display[index] {
                    DisplayRow::Fold(range) => {
                        let response =
                            ui.interact(rect, ui.id().with(("fold", range.start)), Sense::click());
                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                true,
                                format!("Expand {} unchanged lines", range.len()),
                            )
                        });
                        ui.painter()
                            .rect_filled(rect.shrink2(vec2(0.0, 2.0)), 0, p.surface);
                        ui.painter().text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            format!(
                                "···  {} unchanged lines  ·  click to expand  ···",
                                range.len()
                            ),
                            FontId::proportional(11.0),
                            p.muted,
                        );
                        if response.clicked() {
                            expanded = Some(range.start - 3);
                        }
                    }
                    DisplayRow::Line(row_index) => {
                        let row = &c.diff.rows[*row_index];
                        if let Some(h) = row.hunk {
                            visible_hunks.insert(h);
                        }
                        for side in 0..2 {
                            let x = full.left() + if side == 0 { 0.0 } else { pane + RAIL };
                            let cell = Rect::from_min_size(pos2(x, y), vec2(pane, ROW));
                            let (doc, syntax, line, inline) = if side == 0 {
                                (&c.left, &c.left_syntax, row.left, &row.left_inline)
                            } else {
                                (&c.right, &c.right_syntax, row.right, &row.right_inline)
                            };
                            let changed = row.kind != ChangeType::Equal;
                            if changed && line.is_some() {
                                ui.painter().rect_filled(
                                    cell,
                                    0,
                                    if side == 0 { p.remove_wash } else { p.add_wash },
                                );
                                ui.painter().text(
                                    pos2(x + 12.0, y + ROW / 2.0),
                                    Align2::CENTER_CENTER,
                                    if side == 0 { "−" } else { "+" },
                                    FontId::monospace(12.0),
                                    if side == 0 { p.removed } else { p.added },
                                );
                            }
                            if row.hunk == Some(self.current) {
                                ui.painter().rect_filled(
                                    Rect::from_min_size(pos2(x, y), vec2(2.0, ROW)),
                                    0,
                                    p.accent,
                                );
                            }
                            if let Some(line) = line {
                                ui.painter().text(
                                    pos2(x + 49.0, y + ROW / 2.0),
                                    Align2::RIGHT_CENTER,
                                    (line + 1).to_string(),
                                    FontId::monospace(11.0),
                                    p.muted,
                                );
                                let text_rect =
                                    Rect::from_min_max(pos2(x + 62.0, y), cell.right_bottom());
                                let clip = text_rect.intersect(ui.clip_rect());
                                let job = line_job(
                                    doc,
                                    syntax,
                                    line,
                                    inline,
                                    dark,
                                    if side == 0 {
                                        p.remove_inline
                                    } else {
                                        p.add_inline
                                    },
                                    p.text,
                                    self.prefs.font_size,
                                );
                                let galley = ui.fonts_mut(|f| f.layout_job(job));
                                ui.painter().with_clip_rect(clip).galley(
                                    pos2(
                                        text_rect.left() - self.horizontal,
                                        y + (ROW - galley.size().y) / 2.0,
                                    ),
                                    galley,
                                    p.text,
                                );
                                let response = ui.interact(
                                    cell,
                                    ui.id().with(("line", side, line)),
                                    Sense::click(),
                                );
                                response.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::WidgetType::Label,
                                        true,
                                        format!(
                                            "{} line {}: {}",
                                            if side == 0 { "Original" } else { "Changed" },
                                            line + 1,
                                            doc.display_line(line)
                                        ),
                                    )
                                });
                                if response.clicked()
                                    && let Some(h) = row.hunk
                                {
                                    clicked_hunk = Some(h);
                                }
                                response.context_menu(|ui| {
                                    if ui.button("Copy line").clicked() {
                                        ui.ctx().copy_text(doc.line(line).into());
                                        ui.close();
                                    }
                                    if let Some(h) = row.hunk
                                        && ui.button("Copy changed block").clicked()
                                    {
                                        let range = if side == 0 {
                                            &c.diff.hunks[h].left_range
                                        } else {
                                            &c.diff.hunks[h].right_range
                                        };
                                        ui.ctx().copy_text(
                                            range.clone().map(|i| doc.line(i)).collect(),
                                        );
                                        ui.close();
                                    }
                                });
                            } else {
                                ui.painter().line_segment(
                                    [
                                        pos2(x + 62.0, y + ROW / 2.0),
                                        pos2(x + pane - 12.0, y + ROW / 2.0),
                                    ],
                                    Stroke::new(0.5, p.border),
                                );
                            }
                        }
                    }
                }
            }
            for h in visible_hunks {
                let hunk = &c.diff.hunks[h];
                let top = full.top() + self.hunk_positions[h] as f32 * ROW;
                let height = hunk.rows.len() as f32 * ROW;
                let left_height = hunk.left_range.len() as f32 * ROW;
                let right_height = hunk.right_range.len() as f32 * ROW;
                let color = if h == self.current {
                    p.accent
                } else {
                    match hunk.change_type {
                        ChangeType::Added => p.added,
                        ChangeType::Removed => p.removed,
                        ChangeType::Modified => p.modified,
                        ChangeType::Equal => p.muted,
                    }
                };
                ribbon(
                    ui.painter(),
                    rail_x,
                    top,
                    height,
                    left_height,
                    right_height,
                    color,
                );
            }
        });
        if !view_rect.is_negative() && !self.display.is_empty() {
            let overview = Rect::from_min_max(
                pos2(view_rect.right() - 12.0, view_rect.top() + 4.0),
                pos2(view_rect.right() - 3.0, view_rect.bottom() - 4.0),
            );
            for (h, hunk) in c.diff.hunks.iter().enumerate() {
                let y = overview.top()
                    + hunk.rows.start as f32 / c.diff.rows.len().max(1) as f32 * overview.height();
                let marker = Rect::from_min_size(pos2(overview.left(), y), vec2(5.0, 4.0));
                ui.painter().rect_filled(
                    marker,
                    1,
                    if h == self.current {
                        p.accent
                    } else {
                        match c.diff.hunks[h].change_type {
                            ChangeType::Added => p.added,
                            ChangeType::Removed => p.removed,
                            ChangeType::Modified => p.modified,
                            ChangeType::Equal => p.muted,
                        }
                    },
                );
            }
            let response = ui.interact(
                overview.expand(4.0),
                ui.id().with("overview"),
                Sense::click(),
            );
            if response.clicked()
                && let Some(pos) = response.interact_pointer_pos()
            {
                let target = ((pos.y - overview.top()) / overview.height()
                    * c.diff.rows.len() as f32) as usize;
                clicked_hunk = self
                    .comparison
                    .as_ref()
                    .unwrap()
                    .diff
                    .hunks
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, hunk)| hunk.rows.start.abs_diff(target))
                    .map(|(h, _)| h);
                self.jump = true;
            }
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new("Horizontal").size(10.0).color(p.muted));
            ui.add(
                egui::Slider::new(&mut self.horizontal, 0.0..=max_horizontal)
                    .show_value(false)
                    .trailing_fill(true),
            );
            ui.label(
                RichText::new("Right-click a line to copy")
                    .size(10.0)
                    .color(p.muted),
            );
            if self.focus && ui.small_button("Exit Focus  ·  Esc").clicked() {
                self.focus = false;
            }
        });
        if let Some(h) = clicked_hunk {
            self.current = h;
        }
        if let Some(start) = expanded {
            self.expanded.insert(start);
            self.project();
        }
    }
}
impl Diffusion {
    fn snapshot(&self) -> Option<Scratchpad> {
        self.comparison.as_ref().map(|c| Scratchpad {
            names: [c.left.name(), c.right.name()],
            texts: [c.left.text.clone(), c.right.text.clone()],
        })
    }

    fn remember(&mut self) {
        if let Some(content) = self.snapshot() {
            match self.history.record(content, self.history_id) {
                Ok(id) => {
                    self.history_id = Some(id);
                    self.history_notice = None;
                }
                Err(message) => {
                    self.history_id = None;
                    self.history_notice = Some(message);
                }
            }
        }
    }

    fn pin_current(&mut self) {
        if !self
            .history_id
            .is_some_and(|id| self.history.entries.iter().any(|e| e.id == id))
        {
            self.remember();
        }
        if let Some(id) = self.history_id {
            self.history.toggle_pin(id);
        }
    }

    fn open_editor(&mut self, edit: bool) {
        if edit && let Some(content) = self.snapshot() {
            self.scratchpad = content;
            self.editor_origin = self.history_id;
        } else {
            self.editor_origin = None;
        }
        self.editor_error = None;
        self.editor_open = true;
        self.history_open = false;
        self.focus = false;
    }

    fn valid_scratchpad(content: &Scratchpad) -> bool {
        content.texts.iter().all(|text| {
            text.len() as u64 <= MAX_FILE_BYTES
                && text.bytes().filter(|b| *b == b'\n').count() <= 250_000
        })
    }

    fn compare_scratchpad(&mut self) {
        if !Self::valid_scratchpad(&self.scratchpad) {
            self.editor_error = Some("Each side must fit within 16 MiB and 250,000 lines.".into());
            return;
        }
        let content = self.scratchpad.clone();
        let origin = self.editor_origin;
        self.load_snapshot(content, origin);
        self.editor_open = false;
    }

    fn load_snapshot(&mut self, content: Scratchpad, origin: Option<u64>) {
        self.new_comparison();
        self.history_id = origin;
        self.inputs = std::array::from_fn(|i| {
            Some(Input::Scratchpad {
                name: if content.names[i].trim().is_empty() {
                    format!("Scratchpad {}.txt", if i == 0 { "A" } else { "B" })
                } else {
                    content.names[i].clone()
                },
                text: content.texts[i].clone(),
            })
        });
        self.history_open = false;
        self.focus = false;
        self.request();
    }

    fn scratchpad_window(&mut self, ctx: &egui::Context, p: Palette) {
        let mut open = true;
        let mut compare = false;
        egui::Window::new("Scratchpad")
            .id(egui::Id::new("scratchpad_window"))
            .open(&mut open)
            .default_size(vec2(920.0, 560.0))
            .min_width(400.0)
            .show(ctx, |ui| {
                ui.label(RichText::new("Edit or paste text on either side. Add a filename extension for syntax highlighting.").color(p.muted));
                ui.add_space(8.0);
                let height = (ui.available_height() - 100.0).max(160.0);
                ui.columns(2, |columns| {
                    for (i, column) in columns.iter_mut().enumerate() {
                        let color = if i == 0 { p.removed } else { p.added };
                        column.horizontal(|ui| {
                            ui.label(RichText::new(if i == 0 { "A" } else { "B" }).strong().color(color));
                            ui.add(egui::TextEdit::singleline(&mut self.scratchpad.names[i]).desired_width(ui.available_width()).char_limit(200));
                        });
                        egui::Frame::new().fill(if i == 0 { p.remove_wash } else { p.add_wash })
                            .stroke(Stroke::new(1.0, color.gamma_multiply(0.5)))
                            .corner_radius(6).inner_margin(8).show(column, |ui| {
                                egui::ScrollArea::both().id_salt(("scratchpad_scroll", i)).max_height(height).show(ui, |ui| {
                                    ui.add_sized([ui.available_width(), height], egui::TextEdit::multiline(&mut self.scratchpad.texts[i])
                                        .id(egui::Id::new(("scratchpad_text", i)))
                                        .font(egui::TextStyle::Monospace).desired_width(f32::INFINITY).code_editor());
                                });
                            });
                        column.label(RichText::new(format!("{} bytes", self.scratchpad.texts[i].len())).small().color(p.muted));
                    }
                });
                if let Some(message) = &self.editor_error { ui.colored_label(p.removed, message); }
                ui.horizontal(|ui| {
                    compare = ui.button(RichText::new("Compare text").strong()).clicked();
                    ui.label(RichText::new(platform::shortcut("Enter")).small().color(p.muted));
                    ui.label(RichText::new("Draft saved locally · source files stay unchanged").small().color(p.muted));
                });
            });
        self.editor_open = open;
        if compare || ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)) {
            self.compare_scratchpad();
        }
    }

    fn history_window(&mut self, ctx: &egui::Context, p: Palette) {
        let mut open = true;
        let mut action = None;
        egui::Window::new("Clipboard & scratchpad history")
            .id(egui::Id::new("history_window"))
            .open(&mut open).default_size(vec2(640.0, 460.0))
            .show(ctx, |ui| {
                ui.label(RichText::new("Saved on this device · 20 recent comparisons · pins are kept · 64 MiB total").small().color(p.muted));
                if let Some(message) = &self.history_notice { ui.colored_label(p.removed, message); }
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.history_query).hint_text("Search names or text…").desired_width((ui.available_width() - 115.0).max(120.0)));
                    if ui.button("Clear recent").on_hover_text("Delete unpinned history; keep all pins").clicked() { action = Some((0, 4)); }
                });
                ui.separator();
                let query = self.history_query.to_lowercase();
                let mut visible = 0;
                egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                    for pinned in [true, false] {
                        let mut heading = false;
                        for entry in &self.history.entries {
                            if entry.pinned != pinned || (!query.is_empty() && !entry.content.names.iter().chain(entry.content.texts.iter()).any(|s| s.to_lowercase().contains(&query))) { continue; }
                            if !heading {
                                ui.label(RichText::new(if pinned { "PINNED" } else { "RECENT · newest first" }).small().strong().color(p.modified));
                                heading = true;
                            }
                            visible += 1;
                            ui.push_id(entry.id, |ui| {
                                egui::Frame::new().fill(p.surface).corner_radius(6).inner_margin(10).show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(&entry.content.names[0]).strong().color(p.removed));
                                        ui.label(RichText::new("vs").color(p.muted));
                                        ui.label(RichText::new(&entry.content.names[1]).strong().color(p.added));
                                    });
                                    let preview: String = entry.content.texts[0].lines().find(|l| !l.trim().is_empty()).unwrap_or("(empty A)").chars().take(80).collect();
                                    ui.label(RichText::new(preview).monospace().small().color(p.muted));
                                    ui.horizontal(|ui| {
                                        if ui.button("Open").clicked() { action = Some((entry.id, 0)); }
                                        if ui.button("Edit").clicked() { action = Some((entry.id, 1)); }
                                        if ui.button(if pinned { "Unpin" } else { "Pin" }).clicked() { action = Some((entry.id, 2)); }
                                        if ui.small_button("Delete").clicked() { action = Some((entry.id, 3)); }
                                        ui.label(RichText::new(format!("{} bytes", entry.content.texts.iter().map(String::len).sum::<usize>())).small().color(p.muted));
                                    });
                                });
                            });
                            ui.add_space(6.0);
                        }
                    }
                });
                if visible == 0 {
                    ui.add_space(20.0);
                    ui.label(if query.is_empty() { "Paste two texts or compare a scratchpad to save your first comparison." } else { "No comparisons match your search." });
                }
            });
        self.history_open = open;
        if let Some((id, operation)) = action {
            match operation {
                0 | 1 => {
                    if let Some(content) = self
                        .history
                        .entries
                        .iter()
                        .find(|e| e.id == id)
                        .map(|e| e.content.clone())
                    {
                        if operation == 0 {
                            self.load_snapshot(content, Some(id));
                        } else {
                            self.scratchpad = content;
                            self.open_editor(false);
                            self.editor_origin = Some(id);
                        }
                    }
                }
                2 => self.history.toggle_pin(id),
                3 => self.history.remove(id),
                4 => self.history.clear_recent(),
                _ => {}
            }
        }
    }

    fn render(&mut self, root: &mut egui::Ui) {
        let ctx = root.ctx().clone();
        while let Ok(reply) = self.worker.replies.try_recv() {
            if reply.generation == self.generation {
                self.loading = false;
                match reply.result {
                    Ok(c) => {
                        self.comparison = Some(c);
                        if !self.demo
                            && (self.history_id.is_some()
                                || self.inputs.iter().any(|input| {
                                    matches!(
                                        input,
                                        Some(Input::Clipboard { .. } | Input::Scratchpad { .. })
                                    )
                                }))
                        {
                            self.remember();
                        }
                        self.expanded.clear();
                        self.current = 0;
                        self.horizontal = 0.0;
                        self.project();
                        self.jump = true;
                    }
                    Err(e) => self.error = Some(e),
                }
            }
        }
        let dark = match self.prefs.theme {
            1 => false,
            2 => true,
            _ => ctx.input(|i| i.raw.system_theme == Some(egui::Theme::Dark)),
        };
        let p = Palette::get(dark);
        p.apply(&ctx, dark);
        #[cfg(feature = "screenshot")]
        if self.capture_frames == 0
            && self.comparison.is_some()
            && std::env::var_os("DIFFUSION_CAPTURE").is_some()
        {
            match std::env::var("DIFFUSION_CAPTURE_VIEW").as_deref() {
                Ok("scratchpad") => self.open_editor(true),
                Ok("history") => {
                    self.remember();
                    if let Some(id) = self.history_id
                        && !self.history.entries.iter().any(|e| e.id == id && e.pinned)
                    {
                        self.history.toggle_pin(id);
                    }
                    self.history_open = true;
                }
                _ => {}
            }
        }
        self.shortcuts(&ctx);
        while let Ok(action) = self.menu.events.try_recv() {
            match action.as_str() {
                "new" => self.new_comparison(),
                "open" => self.pick(None),
                "close" => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                "preferences" => self.settings = true,
                "next" => self.navigate(true),
                "previous" => self.navigate(false),
                "scratchpad" => {
                    self.scratchpad = Scratchpad::blank();
                    self.open_editor(false);
                }
                "edit" => self.open_editor(true),
                "history" => self.history_open = true,
                _ => {}
            }
        }
        let dropped = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect::<Vec<_>>()
        });
        if !dropped.is_empty() {
            let side = if self.comparison.is_some() {
                ctx.input(|i| {
                    i.pointer
                        .hover_pos()
                        .map(|p| usize::from(p.x > root.max_rect().center().x))
                })
            } else if self.inputs.iter().any(Option::is_some) {
                // A second drop completes the pair, including on an occupied target.
                None
            } else {
                ctx.input(|i| {
                    i.pointer
                        .hover_pos()
                        .and_then(|pos| self.drop_targets.iter().position(|r| r.contains(pos)))
                })
            };
            self.accept_paths(dropped, side);
        }
        if !self.focus {
            egui::Panel::top("toolbar")
                .frame(egui::Frame::NONE)
                .show(root, |ui| self.toolbar(ui, p));
            if self.comparison.is_some() {
                egui::Panel::top("files")
                    .frame(egui::Frame::new().fill(p.background))
                    .show(root, |ui| self.file_headers(ui, p));
                egui::Panel::bottom("status")
                    .frame(egui::Frame::NONE)
                    .show(root, |ui| self.status(ui, p));
            }
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(p.background))
            .show(root, |ui| {
                if let Some(error) = self.error.clone() {
                    egui::Frame::new()
                        .fill(p.remove_wash)
                        .inner_margin(12)
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(RichText::new(error).color(p.removed));
                                if ui.small_button("Dismiss").clicked() {
                                    self.error = None;
                                }
                            });
                        });
                }
                if self.comparison.is_some() {
                    self.comparison_view(ui, p);
                } else {
                    self.welcome(ui, p);
                }
            });
        if self.settings {
            self.preferences(&ctx);
        }
        if self.editor_open {
            self.scratchpad_window(&ctx, p);
        }
        if self.history_open {
            self.history_window(&ctx, p);
        }
        #[cfg(feature = "screenshot")]
        if let Ok(path) = std::env::var("DIFFUSION_CAPTURE") {
            if !self.loading
                && (std::env::var_os("DIFFUSION_CAPTURE_WAIT").is_none()
                    || self.comparison.is_some())
            {
                self.capture_frames += 1;
                if self.capture_frames
                    >= std::env::var("DIFFUSION_CAPTURE_FRAMES")
                        .ok()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(8)
                {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                }
                ctx.request_repaint();
            }
            let screenshot = ctx.input(|i| {
                i.events.iter().find_map(|event| {
                    if let egui::Event::Screenshot { image, .. } = event {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
            });
            if let Some(image) = screenshot {
                let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                image::save_buffer(
                    &path,
                    &bytes,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                )
                .expect("save screenshot");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("drop"),
            ));
            let rect = root.max_rect().shrink(8.0);
            painter.rect_filled(rect, 8, p.background.gamma_multiply(0.94));
            painter.rect_stroke(
                rect,
                8,
                Stroke::new(2.0, p.accent),
                egui::StrokeKind::Inside,
            );
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                "Drop to compare",
                FontId::proportional(28.0),
                p.accent,
            );
        }
    }
}
impl eframe::App for Diffusion {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.render(root);
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "preferences", &self.prefs);
        eframe::set_value(storage, "comparison_history_v1", &self.history);
        if Self::valid_scratchpad(&self.scratchpad) {
            eframe::set_value(storage, "scratchpad_v1", &self.scratchpad);
        }
    }
    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(5)
    }
}

#[allow(clippy::too_many_arguments)] // Explicit text and appearance inputs keep this renderer stateless.
fn line_job(
    doc: &Document,
    syntax: &Syntax,
    line: usize,
    inline: &[InlineChange],
    dark: bool,
    highlight: Color32,
    fallback: Color32,
    size: f32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let text = doc.display_line(line);
    let spans = syntax.lines.get(line);
    let mut span_index = 0;
    let mut inline_index = 0;
    for (column, (offset, ch)) in text.char_indices().enumerate() {
        if column >= 4096 {
            job.append(
                " … [line display limited]",
                0.0,
                egui::TextFormat {
                    font_id: FontId::monospace(size),
                    color: fallback,
                    ..Default::default()
                },
            );
            break;
        }
        let color = if let Some(spans) = spans {
            while span_index + 1 < spans.len() && spans[span_index].bytes.end <= offset {
                span_index += 1;
            }
            spans
                .get(span_index)
                .map(|s| {
                    let c = if dark { s.dark } else { s.light };
                    Color32::from_rgb(c[0], c[1], c[2])
                })
                .unwrap_or(fallback)
        } else {
            fallback
        };
        while inline_index < inline.len() && inline[inline_index].bytes.end <= offset {
            inline_index += 1;
        }
        let changed = inline
            .get(inline_index)
            .is_some_and(|s| s.bytes.contains(&offset));
        let text = if ch == '\t' {
            "    ".into()
        } else {
            ch.to_string()
        };
        job.append(
            &text,
            0.0,
            egui::TextFormat {
                font_id: FontId::monospace(size),
                color,
                background: if changed {
                    highlight
                } else {
                    Color32::TRANSPARENT
                },
                ..Default::default()
            },
        );
    }
    if !doc.line(line).ends_with('\n') {
        job.append(
            "  ¬",
            0.0,
            egui::TextFormat {
                font_id: FontId::monospace(11.0),
                color: fallback.gamma_multiply(0.5),
                ..Default::default()
            },
        );
    }
    job
}
fn mark(p: &egui::Painter, center: Pos2, radius: f32, left_color: Color32, right_color: Color32) {
    for (direction, color) in [(-1.0, left_color), (1.0, right_color)] {
        let points = [
            center + vec2(-radius * 0.7, -radius),
            center + vec2(radius * direction, -radius * 0.2),
            center + vec2(-radius * direction, radius * 0.2),
            center + vec2(radius * 0.7, radius),
        ];
        p.add(egui::epaint::CubicBezierShape::from_points_stroke(
            points,
            false,
            Color32::TRANSPARENT,
            Stroke::new(2.0, color),
        ));
    }
}

fn spectrum_line(
    painter: &egui::Painter,
    start: Pos2,
    end: Pos2,
    left: Color32,
    middle: Color32,
    right: Color32,
) {
    let center = pos2((start.x + end.x) * 0.5, start.y);
    let mut mesh = egui::Mesh::default();
    let colors = [left, middle, right];
    let points = [start, center, end];
    for i in 0..3 {
        mesh.colored_vertex(points[i], colors[i]);
        mesh.colored_vertex(points[i] + vec2(0.0, 2.0), colors[i].gamma_multiply(0.35));
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    mesh.add_triangle(2, 3, 4);
    mesh.add_triangle(3, 5, 4);
    painter.add(egui::Shape::mesh(mesh));
}
fn ribbon(p: &egui::Painter, x: f32, top: f32, height: f32, left: f32, right: f32, color: Color32) {
    let (lt, lb) = if left == 0.0 {
        (top + height / 2.0 - 2.0, top + height / 2.0 + 2.0)
    } else {
        (top + 2.0, top + left - 2.0)
    };
    let (rt, rb) = if right == 0.0 {
        (top + height / 2.0 - 2.0, top + height / 2.0 + 2.0)
    } else {
        (top + 2.0, top + right - 2.0)
    };
    let mut mesh = egui::Mesh::default();
    let (mut upper, mut lower) = (Vec::new(), Vec::new());
    for i in 0..=20 {
        let t = i as f32 / 20.0;
        let ease = t * t * (3.0 - 2.0 * t);
        let a = pos2(x + RAIL * t, lt + (rt - lt) * ease);
        let b = pos2(x + RAIL * t, lb + (rb - lb) * ease);
        upper.push(a);
        lower.push(b);
        mesh.colored_vertex(a, color.gamma_multiply(0.12));
        mesh.colored_vertex(b, color.gamma_multiply(0.12));
        if i > 0 {
            let k = (i * 2) as u32;
            mesh.add_triangle(k - 2, k - 1, k);
            mesh.add_triangle(k - 1, k + 1, k);
        }
    }
    p.add(egui::Shape::mesh(mesh));
    p.add(egui::Shape::line(
        upper,
        Stroke::new(0.8, color.gamma_multiply(0.5)),
    ));
    p.add(egui::Shape::line(
        lower,
        Stroke::new(0.8, color.gamma_multiply(0.5)),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    #[derive(Debug)]
    struct DropPath(PathBuf);
    impl egui::DroppedFile for DropPath {
        fn path(&self) -> &std::path::Path {
            &self.0
        }
        fn bytes(&self) -> Result<Vec<u8>, String> {
            std::fs::read(&self.0).map_err(|e| e.to_string())
        }
    }
    fn app(ctx: &egui::Context) -> Diffusion {
        Diffusion {
            inputs: [None, None],
            comparison: None,
            worker: Worker::new(ctx.clone()),
            generation: 0,
            loading: false,
            error: None,
            prefs: Preferences::default(),
            options: DiffOptions::default(),
            settings: false,
            current: 0,
            expanded: HashSet::new(),
            display: vec![],
            hunk_positions: vec![],
            jump: false,
            horizontal: 0.0,
            demo: false,
            focus: false,
            menu: platform::NativeMenu::new(ctx),
            drop_targets: [Rect::NOTHING; 2],
            history: History::default(),
            history_open: false,
            history_query: String::new(),
            history_id: None,
            history_notice: None,
            scratchpad: Scratchpad::blank(),
            editor_open: false,
            editor_origin: None,
            editor_error: None,
            #[cfg(feature = "screenshot")]
            capture_frames: 0,
        }
    }
    fn frame(
        ctx: &egui::Context,
        app: &mut Diffusion,
        paths: &[PathBuf],
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1220.0, 820.0))),
            dropped_files: paths
                .iter()
                .map(|p| Arc::new(DropPath(p.clone())) as egui::DroppedFileHandle)
                .collect(),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| app.render(ui));
        output.textures_delta.clear();
        output
    }
    fn wait(ctx: &egui::Context, app: &mut Diffusion) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while app.loading && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
            frame(ctx, app, &[], vec![]);
        }
        assert!(!app.loading, "worker timed out");
        assert!(app.error.is_none(), "{:?}", app.error);
    }
    fn fixtures() -> [PathBuf; 2] {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        [base.join("before.rs"), base.join("after.rs")]
    }
    #[test]
    fn sequential_drop_navigation_theme_and_close() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        let files = fixtures();
        frame(&ctx, &mut app, &files[..1], vec![]);
        assert!(app.inputs[0].is_some());
        assert!(app.comparison.is_none());
        frame(&ctx, &mut app, &files[1..], vec![]);
        wait(&ctx, &mut app);
        assert!(app.hunk_positions.len() >= 3);
        let key = |key, modifiers| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        frame(
            &ctx,
            &mut app,
            &[],
            vec![key(egui::Key::G, egui::Modifiers::COMMAND)],
        );
        assert_eq!(app.current, 1);
        frame(
            &ctx,
            &mut app,
            &[],
            vec![key(
                egui::Key::G,
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            )],
        );
        assert_eq!(app.current, 0);
        app.prefs.theme = 2;
        frame(&ctx, &mut app, &[], vec![]);
        assert!(ctx.global_style().visuals.dark_mode);
        app.prefs.theme = 1;
        frame(&ctx, &mut app, &[], vec![]);
        assert!(!ctx.global_style().visuals.dark_mode);
        let out = frame(
            &ctx,
            &mut app,
            &[],
            vec![key(egui::Key::W, egui::Modifiers::COMMAND)],
        );
        assert!(out.viewport_output.values().any(|v| {
            v.commands
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::Close))
        }));
    }
    #[test]
    fn second_drop_completes_pair_on_occupied_target() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        let files = fixtures();
        frame(&ctx, &mut app, &[], vec![]);
        let target = app.drop_targets[1].center();
        frame(
            &ctx,
            &mut app,
            &files[..1],
            vec![egui::Event::PointerMoved(target)],
        );
        assert!(app.inputs[1].is_some());
        assert!(app.inputs[0].is_none());
        frame(
            &ctx,
            &mut app,
            &files[1..],
            vec![egui::Event::PointerMoved(target)],
        );
        wait(&ctx, &mut app);
        assert!(app.comparison.is_some());
        assert_eq!(app.inputs[0], Some(Input::File(files[1].clone())));
        assert_eq!(app.inputs[1], Some(Input::File(files[0].clone())));
    }
    #[test]
    fn paired_drop_and_failed_replacement_preserve_last_result() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        let files = fixtures();
        frame(&ctx, &mut app, &files, vec![]);
        wait(&ctx, &mut app);
        let name = app.comparison.as_ref().unwrap().left.name();
        app.accept_paths(vec![PathBuf::from("/nonexistent/diffusion.txt")], None);
        assert!(app.error.is_some());
        assert_eq!(app.comparison.as_ref().unwrap().left.name(), name);
        app.error = None;
        app.example();
        wait(&ctx, &mut app);
        app.accept_paths(vec![files[0].clone()], None);
        assert!(!app.demo);
        assert!(app.inputs[1].is_none());
        assert!(app.comparison.is_none());
    }
    #[test]
    fn consecutive_pastes_fill_a_then_b_and_start_a_new_pair() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);

        frame(
            &ctx,
            &mut app,
            &[],
            vec![egui::Event::Paste("first\nclipboard\n".into())],
        );
        assert_eq!(
            app.inputs[0],
            Some(Input::Clipboard {
                name: "Clipboard A".into(),
                text: "first\nclipboard\n".into(),
            })
        );
        assert!(app.inputs[1].is_none());
        assert!(app.comparison.is_none());

        frame(
            &ctx,
            &mut app,
            &[],
            vec![egui::Event::Paste("second\nclipboard\n".into())],
        );
        wait(&ctx, &mut app);
        let comparison = app.comparison.as_ref().unwrap();
        assert_eq!(comparison.left.text, "first\nclipboard\n");
        assert_eq!(comparison.right.text, "second\nclipboard\n");
        assert_eq!(comparison.left.name(), "Clipboard A");
        assert_eq!(comparison.right.name(), "Clipboard B");

        frame(
            &ctx,
            &mut app,
            &[],
            vec![egui::Event::Paste("new pair\n".into())],
        );
        assert!(app.comparison.is_none());
        assert_eq!(
            app.inputs[0],
            Some(Input::Clipboard {
                name: "Clipboard A".into(),
                text: "new pair\n".into(),
            })
        );
        assert!(app.inputs[1].is_none());
    }
    #[test]
    fn worker_discards_stale_comparison() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        let files = fixtures();
        app.accept_paths(files.to_vec(), None);
        app.accept_paths(vec![files[0].clone(), files[0].clone()], None);
        wait(&ctx, &mut app);
        assert!(app.comparison.as_ref().unwrap().diff.hunks.is_empty());
    }

    #[derive(Default)]
    struct MemoryStorage(std::collections::HashMap<String, String>);
    impl eframe::Storage for MemoryStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.into(), value);
        }
        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
        }
        fn flush(&mut self) {}
    }

    #[test]
    fn mixed_clipboard_history_persists_and_reopens_exact_text() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        app.accept_paths(vec![fixtures()[0].clone()], None);
        app.accept_clipboard("clipboard version\n".into());
        wait(&ctx, &mut app);
        let snapshot = app.snapshot().unwrap();
        app.pin_current();
        let mut storage = MemoryStorage::default();
        eframe::App::save(&mut app, &mut storage);
        let restored: History = eframe::get_value(&storage, "comparison_history_v1").unwrap();
        assert_eq!(restored.entries.len(), 1);
        assert!(restored.entries[0].pinned);
        assert_eq!(restored.entries[0].content, snapshot);
        app.new_comparison();
        // Reopening uses content rather than re-reading file inputs.
        app.load_snapshot(
            restored.entries[0].content.clone(),
            Some(restored.entries[0].id),
        );
        wait(&ctx, &mut app);
        assert_eq!(
            app.comparison.as_ref().unwrap().left.text,
            snapshot.texts[0]
        );
        assert_eq!(
            app.comparison.as_ref().unwrap().right.text,
            snapshot.texts[1]
        );
        assert!(
            app.inputs
                .iter()
                .all(|i| matches!(i, Some(Input::Scratchpad { .. })))
        );
    }

    #[test]
    fn paste_in_editor_stays_in_the_focused_pane_and_draft_persists() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        app.open_editor(false);
        frame(&ctx, &mut app, &[], vec![]);
        ctx.memory_mut(|memory| memory.request_focus(egui::Id::new(("scratchpad_text", 1_usize))));
        frame(
            &ctx,
            &mut app,
            &[],
            vec![egui::Event::Paste("pasted into B\n".into())],
        );
        assert_eq!(app.scratchpad.texts[1], "pasted into B\n");
        assert!(app.scratchpad.texts[0].is_empty());
        assert!(app.inputs.iter().all(Option::is_none));
        let mut storage = MemoryStorage::default();
        eframe::App::save(&mut app, &mut storage);
        let restored: Scratchpad = eframe::get_value(&storage, "scratchpad_v1").unwrap();
        assert_eq!(restored, app.scratchpad);
        app.compare_scratchpad();
        wait(&ctx, &mut app);
        assert!(app.comparison.as_ref().unwrap().left.text.is_empty());
        assert_eq!(app.history.entries.len(), 1);
    }

    #[test]
    fn editing_a_pin_creates_new_history_and_keeps_original() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        app.accept_clipboard("old A".into());
        app.accept_clipboard("old B".into());
        wait(&ctx, &mut app);
        app.pin_current();
        let pinned_id = app.history_id.unwrap();
        app.open_editor(true);
        app.scratchpad.names[1] = "changed.json".into();
        app.scratchpad.texts[1] = "{\"new\":true}".into();
        app.compare_scratchpad();
        wait(&ctx, &mut app);
        assert_eq!(
            app.comparison.as_ref().unwrap().right.name(),
            "changed.json"
        );
        assert_ne!(app.history_id, Some(pinned_id));
        assert_eq!(
            app.history
                .entries
                .iter()
                .find(|e| e.id == pinned_id)
                .unwrap()
                .content
                .texts[1],
            "old B"
        );
        assert_eq!(app.history.entries.len(), 2);
        app.open_editor(true);
        app.scratchpad.texts[0] = "too big\n".repeat(250_001);
        app.compare_scratchpad();
        assert!(app.editor_error.is_some());
        assert!(app.editor_open);
        assert_eq!(app.comparison.as_ref().unwrap().left.text, "old A");
        // Both history windows can render without taking ownership of the source text.
        app.history_open = true;
        app.history_query = "changed.json".into();
        frame(&ctx, &mut app, &[], vec![]);
    }

    #[test]
    fn deleted_current_entry_can_be_pinned_again() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        app.accept_clipboard("A".into());
        app.accept_clipboard("B".into());
        wait(&ctx, &mut app);
        app.history.clear_recent();
        app.pin_current();
        assert_eq!(app.history.entries.len(), 1);
        assert!(app.history.entries[0].pinned);
    }
}
