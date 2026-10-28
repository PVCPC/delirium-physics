use bevy_egui::egui;
use skate_hud::{apt_scene, hom};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub struct Screen {
    json: serde_json::Value,
    root: PathBuf,
    shapes: apt_scene::Shapes,
    files: BTreeMap<String, [u32; 2]>,
    tex: BTreeMap<String, egui::TextureHandle>,
    pub run: Option<hom::Runtime>,
}

impl Screen {
    pub fn load() -> Result<Screen, String> {
        let root = crate::skate_files::dir().join("private/hom");
        let json: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join("runtime/homscoring.json"))
            .map_err(|e| format!("{}: {e}", root.display()))?).map_err(|e| e.to_string())?;
        let shapes: apt_scene::Shapes = serde_json::from_value(json["shapes"].clone()).map_err(|e| e.to_string())?;
        let mut files = BTreeMap::new();
        for t in shapes.values().flatten().filter_map(|s| s.texture.as_ref()) {
            files.insert(t.rgba.clone(), [t.width, t.height]);
        }
        let run = hom::Runtime::load(&json)?;
        for font in run.bindings.movie.text_assets.fonts.values() {
            files.insert(font.texture.clone(), font.size);
            if let Some(fg) = &font.foreground {
                files.insert(fg.texture.clone(), fg.size);
            }
        }
        Ok(Screen { json, root, shapes, files, tex: BTreeMap::new(), run: None })
    }

    pub fn start(&mut self) -> Result<(), String> {
        let mut run = hom::Runtime::load(&self.json)?;
        run.call("Show", vec![hom::Value::Bool(true)])?;
        run.call("SetState", vec![hom::Value::Text("intro".into())])?;
        self.run = Some(run);
        Ok(())
    }

    pub fn step(&mut self, data: hom::Data) -> Result<(), String> {
        let Some(run) = self.run.as_mut() else { return Ok(()) };
        let old = run.bindings.data;
        run.bindings.data = data;
        if (old.score, old.metrics, old.points) != (data.score, data.metrics, data.points) {
            run.call("UpdateScoring", vec![])?;
        }
        run.advance()
    }

    pub fn outro(&mut self) -> Result<(), String> {
        self.run.as_mut().map_or(Ok(()), |r| r.call("SetState", vec![hom::Value::Text("outro".into())]).map(|_| ()))
    }

    pub fn draw(&mut self, ctx: &egui::Context, r: egui::Rect) -> Result<(), String> {
        let Some(run) = self.run.as_ref() else { return Ok(()) };
        let draws = apt_scene::draw(&run.bindings.movie, &run.vm, &self.shapes)?;
        let s = (r.width() / 1280.0).min(r.height() / 720.0);
        let o = r.center() - egui::vec2(640.0, 360.0) * s;
        let paint = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, "hom".into())).with_clip_rect(r);
        for d in draws {
            let tint = d.add[..3].iter().any(|&c| c > 0.001);
            let key = if tint { format!("{}#shape", d.texture) } else { d.texture.clone() };
            let id = if d.texture.is_empty() {
                egui::TextureId::default()
            } else {
                if !self.tex.contains_key(&key) {
                    let Some(&[w, h]) = self.files.get(&d.texture) else { continue };
                    let mut rgba = std::fs::read(self.root.join(&d.texture)).map_err(|e| format!("{}: {e}", d.texture))?;
                    if rgba.len() != (w * h * 4) as usize {
                        return Err(format!("{}: wrong size", d.texture));
                    }
                    if tint {
                        rgba.chunks_mut(4).for_each(|p| p[..3].fill(255));
                    }
                    let img = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
                    self.tex.insert(key.clone(), ctx.load_texture(&key, img, egui::TextureOptions::LINEAR));
                }
                self.tex[&key].id()
            };
            let c = if tint { [d.add[0], d.add[1], d.add[2], d.multiply[3]] } else { d.multiply };
            let m = c.map(|c| (c.clamp(0.0, 1.0) * 255.0) as u8);
            let color = egui::Color32::from_rgba_unmultiplied(m[0], m[1], m[2], m[3]);
            let mut mesh = egui::Mesh::with_texture(id);
            for v in &d.vertices {
                let uv = if d.texture.is_empty() { egui::epaint::WHITE_UV } else { egui::pos2(v.uv[0], v.uv[1]) };
                mesh.vertices.push(egui::epaint::Vertex { pos: o + egui::vec2(v.position[0], v.position[1]) * s, uv, color });
            }
            mesh.indices = (0..d.vertices.len() as u32).collect();
            paint.add(egui::Shape::mesh(mesh));
        }
        Ok(())
    }

    pub fn film(&mut self, ctx: &egui::Context, r: egui::Rect, k: f32) {
        const KEY: &str = "xray#film";
        if !self.tex.contains_key(KEY) {
            let Ok(mut rgba) = std::fs::read(self.root.join("xray.rgba")) else { return };
            if rgba.len() != 512 * 512 * 4 {
                return;
            }
            for p in rgba.chunks_mut(4) {
                let l = (p[0] as f32 + p[1] as f32 + p[2] as f32) / 765.0;
                let (a, c) = if l < 0.5 { (1.0 - 2.0 * l, 0.5) } else { (2.0 * l - 1.0, 1.0) };
                for x in &mut p[..3] {
                    *x = (*x as f32 * c) as u8;
                }
                p[3] = (a * 255.0) as u8;
            }
            let img = egui::ColorImage::from_rgba_unmultiplied([512, 512], &rgba);
            self.tex.insert(KEY.into(), ctx.load_texture(KEY, img, egui::TextureOptions::LINEAR));
        }
        let paint = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, "hom film".into()));
        paint.image(self.tex[KEY].id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::from_white_alpha((k.clamp(0.0, 1.0) * 255.0) as u8));
    }
}
