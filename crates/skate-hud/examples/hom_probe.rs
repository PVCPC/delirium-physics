fn main() {
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(std::env::args().nth(1).unwrap()).unwrap()).unwrap();
    let mut r = skate_hud::hom::Runtime::load(&json).unwrap();
    r.bindings.data = skate_hud::hom::Data { score: 12345.0, metrics: [31.0, 4.2, 1.25, 3.5, 777.0], points: [100.0, 200.0, 300.0, 400.0, 888.0] };
    println!("loaded; missing so far {:?}", r.bindings.missing);
    for (m, a) in [("Show", vec![skate_hud::apt_vm::Value::Bool(true)]), ("SetState", vec![skate_hud::apt_vm::Value::Text("intro".into())]),
                   ("UpdateScoring", vec![]), ("UpdateMetricsScoring", vec![]), ("UpdateBonusScoring", vec![])] {
        let v = r.call(m, a);
        println!("{m}: {:?}  missing {:?}", v.map(|v| format!("{v:?}")), r.bindings.missing);
        r.bindings.missing.clear();
    }
    for _ in 0..120 { r.advance().unwrap(); }
    println!("after 120 frames missing {:?}", r.bindings.missing);
    for (i, o) in r.vm.objects.iter().enumerate() {
        if let Some(t) = o.fields.get("text") { println!("obj {i} text {t:?} name {:?} visible {:?}", o.fields.get("_name"), o.fields.get("_visible")); }
    }
    let shapes: skate_hud::apt_scene::Shapes = serde_json::from_value(json["shapes"].clone()).unwrap();
    for d in skate_hud::apt_scene::draw(&r.bindings.movie, &r.vm, &shapes).unwrap() {
        let (mut lo, mut hi) = ([9e9f32; 2], [-9e9f32; 2]);
        for v in &d.vertices { for i in 0..2 { lo[i] = lo[i].min(v.position[i]); hi[i] = hi[i].max(v.position[i]); } }
        println!("{:<70} mul {:?} add {:?} {:?} box {:?}-{:?}", d.texture.rsplit('/').next().unwrap(), d.multiply, d.add, d.fill, lo, hi);
    }
}
#[allow(dead_code)]
fn _x() {}
