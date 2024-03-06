#![forbid(unsafe_code)]
//#![cfg_attr(not(debug_assertions), deny(warnings))] // Forbid warnings in release builds
#![warn(clippy::all, rust_2018_idioms)]

mod app;
pub use app::VorpalApp;

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions::default();


    // Set up debugging server
    /*
    puffin::set_scopes_on(true); // tell puffin to collect data
    let server_addr = format!("0.0.0.0:{}", puffin_http::DEFAULT_PORT);
    let srv = puffin_http::Server::new(&server_addr).unwrap();
    std::mem::forget(srv);
    */

    eframe::run_native(
        "eframe template",
        native_options,
        Box::new(|cc| Box::new(VorpalApp::new(cc))),
    )
}

// When compiling to web using trunk:
#[cfg(target_arch = "wasm32")]
fn main() {
    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        eframe::WebRunner::new()
            .start(
                "the_canvas_id", // hardcode it
                web_options,
                Box::new(|cc| Box::new(VorpalApp::new(cc))),
            )
            .await
            .expect("failed to start eframe");
    });
}
