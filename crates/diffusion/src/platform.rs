//! Host integrations stay here; the document engine has no OS UI dependencies.
use std::path::PathBuf;

pub fn window_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Diffusion")
            .with_inner_size([1220.0, 820.0])
            .with_min_inner_size([760.0, 500.0])
            .with_app_id("dev.diffusion.Diffusion"),
        renderer: eframe::Renderer::Wgpu,
        #[cfg(target_os = "linux")]
        event_loop_builder: Some(Box::new(|builder| {
            if std::env::var_os("DIFFUSION_X11").is_some() {
                use winit::platform::x11::EventLoopBuilderExtX11;
                builder.with_x11();
            }
        })),
        ..Default::default()
    }
}

pub fn pick_files() -> Option<Vec<PathBuf>> {
    rfd::FileDialog::new()
        .set_title("Diffusion — choose one or two text files")
        .pick_files()
}

pub fn shortcut(key: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("⌘{key}")
    } else {
        format!("Ctrl+{key}")
    }
}

pub struct NativeMenu {
    #[cfg(all(target_os = "macos", not(test)))]
    _menu: muda::Menu,
    pub events: std::sync::mpsc::Receiver<String>,
}
impl NativeMenu {
    pub fn new(ctx: &eframe::egui::Context) -> Self {
        let (tx, events) = std::sync::mpsc::channel();
        #[cfg(all(target_os = "macos", not(test)))]
        {
            use muda::{Menu, MenuItem, PredefinedMenuItem as Standard, Submenu};
            let menu = Menu::new();
            let app = Submenu::with_items(
                "Diffusion",
                true,
                &[
                    &MenuItem::with_id("preferences", "Preferences…", true, None),
                    &Standard::separator(),
                    &Standard::hide(None),
                    &Standard::hide_others(None),
                    &Standard::show_all(None),
                    &Standard::separator(),
                    &Standard::quit(None),
                ],
            )
            .expect("valid application menu");
            let file = Submenu::with_items(
                "File",
                true,
                &[
                    &MenuItem::with_id("open", "Compare Files…", true, None),
                    &MenuItem::with_id("close", "Close Window", true, None),
                ],
            )
            .expect("valid file menu");
            let change = Submenu::with_items(
                "Compare",
                true,
                &[
                    &MenuItem::with_id("previous", "Previous Change", true, None),
                    &MenuItem::with_id("next", "Next Change", true, None),
                ],
            )
            .expect("valid compare menu");
            menu.append_items(&[&app, &file, &change])
                .expect("valid menu bar");
            menu.init_for_nsapp();
            let ctx = ctx.clone();
            muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
                let _ = tx.send(event.id.0);
                ctx.request_repaint();
            }));
            Self {
                _menu: menu,
                events,
            }
        }
        #[cfg(any(not(target_os = "macos"), test))]
        {
            let _ = (ctx, tx);
            Self { events }
        }
    }
}

/// winit 0.30 has no native Wayland file-drop implementation.
/// Keep this capability check local; Linux still supports picker/CLI comparisons.
pub fn file_drop_available() -> bool {
    !cfg!(target_os = "linux")
        || std::env::var_os("WAYLAND_DISPLAY").is_none()
        || std::env::var_os("DIFFUSION_X11").is_some()
}
