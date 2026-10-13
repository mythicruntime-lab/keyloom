#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;
mod audio;
mod exporter;
mod fixtures;
mod library;
mod light_guide;
mod midi;
mod model;
mod parser;
mod runtime;
mod transport;
fn main() {
    app::run();
}
