#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;
mod audio;
mod fixtures;
mod library;
mod midi;
mod model;
mod parser;
mod runtime;
mod transport;
fn main() { app::run(); }
