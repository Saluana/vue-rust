#[cfg(not(target_arch="wasm32"))]
fn main() -> eframe::Result {
 let options=eframe::NativeOptions { viewport:eframe::egui::ViewportBuilder::default().with_inner_size([640.0, 920.0]).with_title("Quick Quote — Vue / Rust"),..Default::default() };
 eframe::run_native("Quick Quote",options,Box::new(|cc| {
  cc.egui_ctx.set_theme(eframe::egui::Theme::Light);
  let mut style=(*cc.egui_ctx.style_of(eframe::egui::Theme::Light)).clone();
  style.spacing.item_spacing=eframe::egui::vec2(10.0,4.0);
  style.spacing.button_padding=eframe::egui::vec2(12.0,5.0);
  style.spacing.text_edit_width=550.0;
  style.text_styles.insert(eframe::egui::TextStyle::Body,eframe::egui::FontId::proportional(15.0));
  style.text_styles.insert(eframe::egui::TextStyle::Button,eframe::egui::FontId::proportional(14.0));
  style.text_styles.insert(eframe::egui::TextStyle::Heading,eframe::egui::FontId::proportional(24.0));
  cc.egui_ctx.set_style_of(eframe::egui::Theme::Light,style);
  Ok(Box::new(vue_rust_demo::app::App::default()))
 }))
}
#[cfg(target_arch="wasm32")]fn main(){}
