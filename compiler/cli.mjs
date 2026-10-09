import {readFileSync,writeFileSync,mkdirSync,existsSync} from 'node:fs';
import {resolve} from 'node:path';
import {compile} from './compile.mjs';
const [input,output]=process.argv.slice(2);
if(!input||!output){console.error('Usage: node compiler/cli.mjs App.vue NEW_OUTPUT_DIR');process.exit(1);}
try {
const code=compile(readFileSync(input,'utf8'),input);
const dir=resolve(output);
if(existsSync(dir)) throw new Error('Output already exists; choose a new directory to preserve your files');
mkdirSync(dir+'/src',{recursive:true});
writeFileSync(dir+'/src/app.rs',code);
writeFileSync(dir+'/Cargo.toml',`[package]\nname = "vue-rust-demo"\nversion = "0.0.1"\nedition = "2024"\n[lib]\ncrate-type = ["cdylib", "rlib"]\n[dependencies]\neframe = { version = "=0.36.2", default-features = false, features = ["default_fonts", "glow", "x11", "wayland", "accesskit"] }\n[target.'cfg(target_arch = "wasm32")'.dependencies]\nwasm-bindgen = "0.2"\nwasm-bindgen-futures = "0.4"\nweb-sys = { version = "0.3", features = ["HtmlCanvasElement"] }\n`);
writeFileSync(dir+'/src/main.rs',`#[cfg(not(target_arch="wasm32"))]\nfn main() -> eframe::Result { eframe::run_native("Vue Rust prototype", eframe::NativeOptions::default(), Box::new(|_| Ok(Box::new(vue_rust_demo::app::App::default())))) }\n#[cfg(target_arch="wasm32")] fn main() {}\n`);
writeFileSync(dir+'/src/lib.rs',`pub mod app;\n#[cfg(target_arch="wasm32")]\nmod web {\nuse wasm_bindgen::prelude::*;\n#[wasm_bindgen]\npub struct WebApp { runner: eframe::WebRunner }\n#[wasm_bindgen]\nimpl WebApp {\n#[wasm_bindgen(constructor)] pub fn new() -> Self { Self { runner: eframe::WebRunner::new() } }\npub async fn start(&self, canvas: web_sys::HtmlCanvasElement) -> Result<(), JsValue> { self.runner.start(canvas, eframe::WebOptions::default(), Box::new(|_| Ok(Box::new(crate::app::App::default())))).await }\npub fn destroy(&self) { self.runner.destroy(); }\n}\n}\n`);
writeFileSync(dir+'/index.html',`<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Vue Rust prototype</title><style>html,body{margin:0;width:100%;height:100%}canvas{width:100%;height:100%;display:block}</style><canvas id="app"></canvas><script type="module">import init,{WebApp} from './pkg/vue_rust_demo.js'; await init(); window.app=new WebApp(); await window.app.start(document.querySelector('#app'));</script></html>`);
console.log(`Generated Rust app in ${dir}`);
} catch(e){console.error(e.message);process.exit(1);}
