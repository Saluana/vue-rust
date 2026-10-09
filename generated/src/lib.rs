pub mod app;
#[cfg(target_arch="wasm32")]
mod web {
use wasm_bindgen::prelude::*;
#[wasm_bindgen]
pub struct WebApp { runner: eframe::WebRunner }
#[wasm_bindgen]
impl WebApp {
#[wasm_bindgen(constructor)] pub fn new() -> Self { Self { runner: eframe::WebRunner::new() } }
pub async fn start(&self, canvas: web_sys::HtmlCanvasElement) -> Result<(), JsValue> { self.runner.start(canvas, eframe::WebOptions::default(), Box::new(|_| Ok(Box::new(crate::app::App::default())))).await }
pub fn destroy(&self) { self.runner.destroy(); }
}
}
