#![forbid(unsafe_code)]
//#![cfg_attr(not(debug_assertions), deny(warnings))] // Forbid warnings in release builds
#![warn(clippy::all, rust_2018_idioms)]

// ----------------------------------------------------------------------------
// When compiling for web:
pub mod file_watcher;
pub mod wasmtime_integration;

pub const TIME_KEY: &str = "Time (seconds)";
pub const POS_KEY: &str = "Position (pixels)";
pub const RESOLUTION_KEY: &str = "Resolution (pixels)";

#[cfg(target_arch = "wasm32")]
use eframe::wasm_bindgen::{self, prelude::*};

/// This is the entry-point for all the web-assembly.
/// This is called once from the HTML.
/// It loads the app, installs some callbacks, then returns.
/// You can add more callbacks like this if you want to call in to your code.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn start(canvas_id: &str) -> Result<(), eframe::wasm_bindgen::JsValue> {
    let app = NodeGraphExample::default();
    eframe::start_web(canvas_id, Box::new(app))
}
