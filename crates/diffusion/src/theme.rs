use eframe::egui::{self, Color32 as C};

#[derive(Clone, Copy)]
pub struct Palette {
    pub background: C,
    pub surface: C,
    pub text: C,
    pub muted: C,
    pub border: C,
    pub accent: C,
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
                surface: c(0x1e242c),
                text: c(0xdce2e9),
                muted: c(0x8d99a8),
                border: c(0x303944),
                accent: c(0xa6aafa),
                added: c(0x78c9b0),
                removed: c(0xe6a18b),
                add_wash: c(0x1d332f),
                remove_wash: c(0x352925),
                add_inline: c(0x305a4d),
                remove_inline: c(0x654235),
            }
        } else {
            Self {
                background: c(0xfaf9f6),
                surface: c(0xf0efec),
                text: c(0x28333e),
                muted: c(0x74808a),
                border: c(0xdedfdc),
                accent: c(0x6264b8),
                added: c(0x277864),
                removed: c(0xa9513e),
                add_wash: c(0xe7f2ec),
                remove_wash: c(0xf7eae3),
                add_inline: c(0xbce0ce),
                remove_inline: c(0xefc6b5),
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
