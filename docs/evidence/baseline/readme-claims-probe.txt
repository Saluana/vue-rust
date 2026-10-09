== A. Compiler behaviour of the counter fixture compiler (README "What works" and "What does not work yet")
[A1] elements main/section/div/h1/h2/p/span/label: ACCEPTED -> pub fn draw(&mut self, ui: &mut egui::Ui) { ui.vertical(|ui| {ui.vertical(|ui| {ui.vertical(|ui| {ui.heading([String::from("A")].concat());
[A2] primitive refs number/string/boolean: ACCEPTED -> pub struct App { pub a: f64, | pub b: String, | pub c: bool, }
[A3] ref assignment count = 5 in a handler: ACCEPTED -> pub fn draw(&mut self, ui: &mut egui::Ui) { if ui.add_enabled(!(false), egui::Button::new([String::from("x")].concat())).clicked() { self.count = 5.0f64; } }
[A4] decrement count-- in a handler: ACCEPTED -> pub fn draw(&mut self, ui: &mut egui::Ui) { if ui.add_enabled(!(false), egui::Button::new([String::from("x")].concat())).clicked() { self.count -= 1.0; } }
[A5] :disabled with strict comparison: ACCEPTED -> pub fn draw(&mut self, ui: &mut egui::Ui) { if ui.add_enabled(!((self.count == 0.0f64)), egui::Button::new([String::from("x")].concat())).clicked() { self.inc(); } }
[A6] strict !== comparison in v-if: ACCEPTED -> pub fn draw(&mut self, ui: &mut egui::Ui) { if (self.count != 0.0f64) {ui.label([String::from("x")].concat());} }
[A7] handler with a parameter (must be zero-argument): REJECTED -> App.vue: Handlers must be synchronous zero-argument functions
[A8] <script setup lang="ts"> with plain TypeScript (no annotations): ACCEPTED -> pub struct App { pub count: f64, }
[A9] custom component <MyCard/> (nested components): REJECTED -> App.vue: Unsupported element <MyCard>
[A10] <slot/> in template: REJECTED -> App.vue: Unsupported element <slot>
[A11] defineProps({}) statement: REJECTED -> App.vue: Unsupported setup syntax: ExpressionStatement
[A12] defineEmits([...]) statement: REJECTED -> App.vue: Unsupported setup syntax: ExpressionStatement
[A13] reactive({}) (import from vue): REJECTED -> App.vue: Only unaliased ref/computed imports are supported
[A14] watch(count, ...) (import from vue): REJECTED -> App.vue: Only unaliased ref/computed imports are supported
[A15] onMounted(...) lifecycle hook: REJECTED -> App.vue: Only unaliased ref/computed imports are supported
[A16] import from vue-router (routing): REJECTED -> App.vue: Only ref and computed imports from vue are supported
[A17] import thing from npm package: REJECTED -> App.vue: Only ref and computed imports from vue are supported
[A18] fetch(...) call (async/network): REJECTED -> App.vue: Unsupported setup syntax: ExpressionStatement
[A19] console.log(...) (DOM/browser API): REJECTED -> App.vue: Unsupported setup syntax: ExpressionStatement
[A20] <style> block (CSS): REJECTED -> App.vue: CSS and custom blocks are not implemented yet; they are never silently dropped
[A21] v-for list directive: REJECTED -> App.vue: Unsupported/duplicate directive for
[A22] TypeScript generic ref<number>(0): REJECTED -> App.vue: Explicit TypeScript types and generics are not implemented yet
[A23] Options API export default: REJECTED -> App.vue: Use one <script setup> and one <template>; Options API is unsupported

== B. Dynamic event argument and interpolated number text (KNOWN BUG cases)
[B1] @[click]="inc" (counter): ACCEPTED -> pub fn draw(&mut self, ui: &mut egui::Ui) { if ui.add_enabled(!(false), egui::Button::new([String::from("x")].concat())).clicked() { self.inc(); } }
[B2] @[evt]="inc" (counter): REJECTED -> App.vue: Unsupported/duplicate directive on:evt
[B3] @[click]="inc" (quick-quote): ACCEPTED -> pub fn draw(&mut self, ui: &mut egui::Ui) { if ui.add_enabled(!(false), egui::Button::new([String::from("x")].concat())).clicked() { self.inc(); } }
[B4] @[evt]="inc" (quick-quote): REJECTED -> App.vue: Unsupported/duplicate directive on:evt
[B5] interpolated ref(1e21) (counter): ACCEPTED -> pub struct App { pub big: f64, } | impl Default for App { fn default() -> Self { Self {big: 1e+21f64,} } } | pub fn draw(&mut self, ui: &mut egui::Ui) { ui.label([format!("{}", self.big)].concat()); }
[B6] toFixed (counter; quick-quote only): REJECTED -> App.vue: Unsupported expression: CallExpression
[B7] toFixed (quick-quote): ACCEPTED -> pub fn a(&self) -> String { format!("{:.0}", 0.5f64) }
[B8] computed doubled (counter): getter shape: ACCEPTED -> pub fn doubled(&self) -> f64 { (self.count * 2.0f64) }

== C. Generated application and CLI (README "Try it", "generated/", evidence/verification.txt L2)
compile(examples/Counter.vue) === generated/src/app.rs: true
cli first run exit=0 stdout="Generated Rust app in /var/folders/1d/hmmp5gtj3pd8bwyqgpzbh5d00000gn/T/vn-g0a-probe-4HVyWU/build-counter-2"
  cli output src/app.rs identical to generated/src/app.rs: true
  cli output src/lib.rs identical to generated/src/lib.rs: true
  cli output src/main.rs identical to generated/src/main.rs: true
  cli output Cargo.toml identical to generated/Cargo.toml: true
  cli output index.html identical to generated/index.html: true
cli second run into the same directory: exit=1 stderr="Output already exists; choose a new directory to preserve your files"
cli main.rs starts eframe host: true
generated/Cargo.lock eframe version: 0.36.2; wasm-bindgen: 0.2.129
generated/Cargo.toml enables accesskit feature: true
generated/Cargo.lock names a JavaScript engine or webview crate: no
generated/tests/runtime.rs #[test] count (static, not run): 4

== D. Toolchain and dependency versions (README "Tool versions")
@vue/compiler-sfc installed version: 3.5.43
@babel/parser installed version: 7.29.9
@vue/compiler-dom installed version: 3.5.43
project rustc (scripts/env.sh): rustc 1.99.0 (b940084d7 2026-09-28) exit=0
node: v26.5.1

== E. Quick Quote compiler (zip prototype) for the same probes
[E1] @[click]="inc" (quick-quote): ACCEPTED -> pub fn draw(&mut self, ui: &mut egui::Ui) { if ui.add_enabled(!(false), egui::Button::new([String::from("x")].concat())).clicked() { self.inc(); } }
[E2] <button @click="inc"> (quick-quote, static baseline): ACCEPTED -> pub fn draw(&mut self, ui: &mut egui::Ui) { if ui.add_enabled(!(false), egui::Button::new([String::from("x")].concat())).clicked() { self.inc(); } }

== F. Platform hosts and recorded test names (README "Platform status", evidence/compiler-tests.txt)
iOS/Android/Xcode/Gradle host files in the project (name search, excluding node_modules, toolchain, target, planning): none
recorded evidence/compiler-tests.txt: 25 test names; live run: 25; identical name list and order: true
