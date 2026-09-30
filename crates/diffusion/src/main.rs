mod app;
mod history;
mod platform;
mod theme;
mod worker;

fn main() -> eframe::Result {
    let paths: Vec<_> = std::env::args_os()
        .skip(1)
        .map(std::path::PathBuf::from)
        .collect();
    if paths.len() > 2 {
        eprintln!("Usage: diffusion [LEFT [RIGHT]]");
        std::process::exit(2);
    }
    eframe::run_native(
        "Diffusion",
        platform::window_options(),
        Box::new(move |cc| Ok(Box::new(app::Diffusion::new(cc, paths)))),
    )
}
