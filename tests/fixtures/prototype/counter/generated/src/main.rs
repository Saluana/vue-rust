#[cfg(not(target_arch="wasm32"))]
fn main() -> eframe::Result { eframe::run_native("Vue Rust prototype", eframe::NativeOptions::default(), Box::new(|_| Ok(Box::new(vue_rust_demo::app::App::default())))) }
#[cfg(target_arch="wasm32")] fn main() {}
