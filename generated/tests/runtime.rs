use vue_rust_demo::app::App;
use eframe::egui;
#[test]
fn state_and_computed_are_native_rust() {
    let mut app=App::default();
    assert_eq!(app.count,0.0);
    assert_eq!(app.doubled(),0.0);
    app.increment();app.increment();
    assert_eq!(app.count,2.0);
    assert_eq!(app.doubled(),4.0);
    app.name="Native".into();assert_eq!(app.greeting(),"Hello, Native");
    app.reset();assert_eq!(app.count,0.0);
}
#[test]
fn components_have_independent_state() {
    let mut first=App::default();let second=App::default();
    first.increment();assert_eq!(second.count,0.0);
}
#[test]
fn headless_renderer_produces_real_paint_shapes() {
    let ctx=egui::Context::default();let mut app=App::default();
    let mut output=ctx.run_ui(egui::RawInput::default(),|ui| { app.draw(ui); });
    assert!(!output.shapes.is_empty());
    // Headless test has no GPU to upload the font texture to.
    output.textures_delta.clear();
}
fn frame(ctx:&egui::Context,app:&mut App,events:Vec<egui::Event>)->Vec<(String,egui::Pos2)> {
    let input=egui::RawInput { screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(800.0,600.0))),events,..Default::default() };
    let mut output=ctx.run_ui(input,|ui|app.draw(ui));
    let texts=output.shapes.iter().filter_map(|s| match &s.shape {
        egui::epaint::Shape::Text(t)=>Some((t.galley.text().to_string(),t.pos+t.galley.rect.size()*0.5)),
        _=>None,
    }).collect();
    output.textures_delta.clear();texts
}
fn click(ctx:&egui::Context,app:&mut App,pos:egui::Pos2) {
    frame(ctx,app,vec![egui::Event::PointerMoved(pos),egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed:true,modifiers:Default::default()}]);
    frame(ctx,app,vec![egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed:false,modifiers:Default::default()}]);
}
#[test]
fn real_pointer_click_updates_ref_and_conditional_rendering() {
    let ctx=egui::Context::default();let mut app=App::default();
    frame(&ctx,&mut app,vec![]);
    let texts=frame(&ctx,&mut app,vec![]);
    let pos=texts.iter().find(|(text,_)|text=="Add one").expect("button label").1;
    for _ in 0..6 {click(&ctx,&mut app,pos);}
    assert_eq!(app.count,6.0);assert_eq!(app.doubled(),12.0);
    let texts=frame(&ctx,&mut app,vec![]);
    assert!(texts.iter().any(|(t,_)|t.contains("more than five")));
    let reset=texts.iter().find(|(t,_)|t=="Reset").unwrap().1;
    click(&ctx,&mut app,reset);assert_eq!(app.count,0.0);
    assert!(!frame(&ctx,&mut app,vec![]).iter().any(|(t,_)|t.contains("more than five")));
}
