//! NoteCraft in the browser.
//!
//! Runs the same [`notecraft_ui_egui::NoteApp`] as the desktop app through eframe's web runner
//! (wgpu: WebGPU where available, WebGL2 otherwise). Build with `trunk build --release` from
//! this directory (output in `dist/web`); `trunk serve --release` serves it on
//! `http://127.0.0.1:8785`.
//!
//! Differences from the desktop app (`docs/gaps.md`): notebooks live in memory (the sample
//! notebook opens; nothing is kept after the tab closes, and the browser asks before leaving
//! once something changed); no folders, backups, exports or control channel; bold needs font
//! files the web build doesn't have. URL flag: `?webgl` forces WebGL2.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
fn main() {
    web::start();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("notecraft-web only runs in the browser: build it with `trunk build --release` in apps/notecraft-web");
}
