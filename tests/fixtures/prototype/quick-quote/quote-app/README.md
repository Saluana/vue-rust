# Quick Quote

A small landscaping estimate calculator built with the Vue-to-Rust prototype.

## Run
Install Node.js and Rust, then from the bundle root:

    npm ci
    npm test
    cargo run --locked --manifest-path quote-app/Cargo.toml

The application source is `examples/Quote.vue`. Its generated Rust lives in `quote-app/src/app.rs`. The host window and its default font/spacing setup are in `quote-app/src/main.rs`; app authors do not need to change those.

Use the preset buttons or double-click a numeric value to type a new value. Numeric values can also be dragged. The estimate updates immediately. Prices are sample CAD amounts before tax, not tax or pricing advice. No customer data is saved, uploaded or sent.

## Verification
- 28 compiler tests passed
- Four quote arithmetic/state tests passed
- Actual Linux native window launched and visually inspected
- Mouse selection of Mowing changed rate from100 to80 and total750 to630
- Keyboard edit of hours6 to3.5 changed labour to280 and total to430
- Screenshots captured directly from that running native window
- This follow-up resolves the earlier native-window verification gap for this Linux example
- No Mac/Windows/mobile or web interaction qualification performed in this follow-up

Prototype extensions: numeric v-model.number inputs with literal range/step, hr separator, bounded toFixed formatting, and scrollable/padded root rendering. Numeric controls are egui DragValue rather than HTML controls. toFixed uses Rust formatting and is not a fully qualified JavaScript rounding implementation. This is a quote helper, not accounting software.

No persistence, PDF export, sending, tax calculation or saved customer records yet. No browser/webview is embedded in the native app. The framework still supports only the documented small Composition API subset, not full Vue or CSS.
