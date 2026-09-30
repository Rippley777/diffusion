use eframe::egui::{self, Color32 as C};

#[derive(Clone, Copy)]
pub struct Palette {
    pub background: C,
    pub surface: C,
    pub text: C,
    pub muted: C,
    pub border: C,
    pub accent: C,
    pub modified: C,
    pub added: C,
    pub removed: C,
    pub add_wash: C,
    pub remove_wash: C,
    pub add_inline: C,
    pub remove_inline: C,
}
impl Palette {
    pub fn get(dark: bool) -> Self {
        if dark {
            Self {
                background: c(0x171c22),
                surface: c(0x1d2430),
                text: c(0xdce2e9),
                muted: c(0x94a3b8),
                border: c(0x344052),
                accent: c(0x9b8cff),
                modified: c(0xb47cff),
                added: c(0x49d6a5),
                removed: c(0xff8b72),
                add_wash: c(0x173b35),
                remove_wash: c(0x422a2d),
                add_inline: c(0x216b59),
                remove_inline: c(0x82483f),
            }
        } else {
            Self {
                background: c(0xfaf9f6),
                surface: c(0xf0efec),
                text: c(0x28333e),
                muted: c(0x74808a),
                border: c(0xd9d9e4),
                accent: c(0x6558c7),
                modified: c(0x8457c7),
                added: c(0x087f67),
                removed: c(0xb94b42),
                add_wash: c(0xe0f4ed),
                remove_wash: c(0xfbe7e2),
                add_inline: c(0xa8dfcd),
                remove_inline: c(0xf3bdb3),
            }
        }
    }
    pub fn apply(self, ctx: &egui::Context, dark: bool) {
        let mut visuals = if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        visuals.panel_fill = self.background;
        visuals.window_fill = self.surface;
        visuals.faint_bg_color = self.surface;
        visuals.extreme_bg_color = self.background;
        visuals.override_text_color = Some(self.text);
        visuals.selection.bg_fill = self.accent.gamma_multiply(0.3);
        visuals.selection.stroke = egui::Stroke::new(1.0, self.accent);
        visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, self.border);
        visuals.widgets.hovered.bg_fill = self.accent.gamma_multiply(0.16);
        visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, self.accent);
        visuals.widgets.active.bg_fill = self.accent.gamma_multiply(0.24);
        visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, self.accent);
        ctx.set_visuals(visuals);
        ctx.global_style_mut(|s| {
            s.spacing.item_spacing = egui::vec2(10.0, 8.0);
            s.spacing.button_padding = egui::vec2(12.0, 7.0);
            s.text_styles
                .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
            s.text_styles
                .insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
        });
    }
}
fn c(hex: u32) -> C {
    C::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}
