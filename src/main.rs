#![windows_subsystem = "windows"]

mod app;
mod platform;
mod tray;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    app::run()
}
