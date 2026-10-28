use crate::{apt_movie::Movie, apt_vm::Vm};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Clone, Deserialize)]
pub struct Vertex {
    pub position: [f32; 2],
    #[serde(default)]
    pub uv: [f32; 2],
}
#[derive(Clone, Deserialize)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub rgba: String,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Fill {
    Line,
    Solid,
    #[default]
    TextureClamped,
    TextureWrapped,
}
impl Fill {
    pub fn textured(self) -> bool {
        matches!(self, Self::TextureClamped | Self::TextureWrapped)
    }
}
#[derive(Clone, Deserialize)]
pub struct Shape {
    #[serde(default)]
    pub texture: Option<Texture>,
    pub triangles: Vec<[Vertex; 3]>,
    pub color: [f32; 4],
    #[serde(default, rename = "render_type")]
    pub fill: Fill,
    #[serde(default)]
    pub bitmap: Option<i32>,
}
pub type Shapes = BTreeMap<i32, Vec<Shape>>;
pub trait ShapeSource {
    fn shape(&self, character: i32) -> Option<&[Shape]>;
    fn texture(&self, _character: i32, shape: &Shape) -> String {
        shape.texture.as_ref().map_or_else(String::new, |t| t.rgba.clone())
    }
    fn retail_unit_colour(&self) -> bool {
        false
    }
}
impl ShapeSource for Shapes {
    fn shape(&self, character: i32) -> Option<&[Shape]> {
        self.get(&character).map(Vec::as_slice)
    }
}
pub struct Draw {
    pub texture: String,
    pub vertices: Vec<Vertex>,
    pub multiply: [f32; 4],
    pub add: [f32; 4],
    pub fill: Fill,
}
fn compose(a: [f32; 6], b: [f32; 6]) -> [f32; 6] {
    [
        a[0] * b[0] + a[2] * b[1],
        a[1] * b[0] + a[3] * b[1],
        a[0] * b[2] + a[2] * b[3],
        a[1] * b[2] + a[3] * b[3],
        a[0] * b[4] + a[2] * b[5] + a[4],
        a[1] * b[4] + a[3] * b[5] + a[5],
    ]
}
fn transform(m: [f32; 6], v: &Vertex) -> Vertex {
    Vertex {
        position: [
            m[0] * v.position[0] + m[2] * v.position[1] + m[4],
            m[1] * v.position[0] + m[3] * v.position[1] + m[5],
        ],
        uv: v.uv,
    }
}
pub fn draw(movie: &Movie, vm: &Vm, shapes: &dyn ShapeSource) -> Result<Vec<Draw>, String> {
    let mut draws = Vec::new();
    visit(
        movie,
        vm,
        shapes,
        movie.root,
        [1., 0., 0., 1., 0., 0.],
        [1.; 4],
        [0.; 4],
        &mut draws,
    )?;
    Ok(draws)
}
fn visit(
    movie: &Movie,
    vm: &Vm,
    shapes: &dyn ShapeSource,
    id: usize,
    parent: [f32; 6],
    pm: [f32; 4],
    pa: [f32; 4],
    out: &mut Vec<Draw>,
) -> Result<(), String> {
    if !vm.get(id, "_visible").truth() {
        return Ok(());
    }
    let instance = &movie.instances[&id];
    let mut matrix = instance
        .placement
        .as_ref()
        .map_or([1., 0., 0., 1., 0., 0.], |p| p.matrix);
    matrix[4] = vm.get(id, "_x").number() as f32;
    matrix[5] = vm.get(id, "_y").number() as f32;
    let matrix = compose(parent, matrix);
    if matrix.iter().any(|x| !x.is_finite()) {
        return Err(format!(
            "Nonfinite HUD transform on character {}",
            instance.character
        ));
    }
    let color = instance
        .placement
        .as_ref()
        .map_or([255, 255, 255, 255, 0, 0, 0, 0], |p| p.color);
    let rgba = [1, 2, 3, 0];
    let mut multiply = std::array::from_fn(|i| pm[i] * color[rgba[i]] as f32 / 255.);
    let mut add = std::array::from_fn(|i| pa[i] + pm[i] * color[4 + rgba[i]] as f32 / 255.);
    let alpha = (vm.get(id, "_alpha").number() as f32 / 100.).clamp(0., 1.);
    multiply[3] *= alpha;
    add[3] *= alpha;
    if multiply[3] <= 0. && add[3] <= 0. {
        return Ok(());
    }
    let character = &movie.characters[&instance.character];
    if character.type_name == "shape" {
        for shape in shapes
            .shape(character.id)
            .ok_or("Missing original shape geometry")?
        {
            if shape.fill == Fill::Line {
                continue;
            }
            let (unit_multiply, unit_add) = if shapes.retail_unit_colour() {
                let colour: [f32; 4] = std::array::from_fn(|i| shape.color[i].mul_add(multiply[i], add[i]));
                if colour[3] <= 0. {
                    continue;
                }
                (colour, [0.; 4])
            } else {
                (std::array::from_fn(|i| multiply[i] * shape.color[i]), add)
            };
            out.push(Draw {
                texture: shapes.texture(character.id, shape),
                fill: shape.fill,
                vertices: shape
                    .triangles
                    .iter()
                    .flatten()
                    .map(|v| transform(matrix, v))
                    .collect(),
                multiply: unit_multiply,
                add: unit_add,
            });
        }
    } else if let Some(text) = &character.text {
        let font = &movie.text_assets.fonts
            [&(text["font_id"].as_i64().ok_or("Invalid text font")? as i32)];
        let passes = if let Some(foreground) = &font.foreground {
            vec![
                (font, true, 0.),
                (&**foreground, false, 1.),
            ]
        } else {
            vec![(font, false, 0.)]
        };
        for (font, shadow, advance_x) in passes {
            let height = text["font_height"].as_f64().ok_or("Invalid text height")? as f32;
            let sx = font.scale[0] * height;
            let sy = font.scale[1] * height;
            let value = vm.get(id, "_displayText").text();
            let bounds = character.bounds.ok_or("Missing text bounds")?;
            let width = font.width(&value, height);
            let alignment = text["alignment"].as_u64().unwrap_or(0);
            let autosize = vm.get(id, "autoSize").text();
            let alignment = if autosize == "left" { 0 } else { alignment };
            let mut x = bounds[0]
                + advance_x
                + font.offset[0] * sx
                + match alignment {
                    1 => bounds[2] - bounds[0] - width,
                    2 => (bounds[2] - bounds[0] - width) * 0.5,
                    _ => 0.,
                };
            let y = bounds[1] + font.offset[1] * sy;
            let mut vertices = Vec::new();
            for c in value.chars() {
                if let Some(g) = font.glyph(c) {
                    let x0 = x + g.x_offset * sx;
                    let y0 = y + (font.ascent - g.y_offset) * sy;
                    let x1 = x0 + g.width * sx;
                    let y1 = y0 + g.height * sy;
                    let [u0, v0, u1, v1] = g.atlas_bounds;
                    let points = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
                    let uvs = [[u0, v0], [u1, v0], [u1, v1], [u0, v1]];
                    for i in [0, 1, 2, 0, 2, 3] {
                        vertices.push(transform(
                            matrix,
                            &Vertex {
                                position: points[i],
                                uv: [
                                    uvs[i][0] / font.size[0] as f32,
                                    uvs[i][1] / font.size[1] as f32,
                                ],
                            },
                        ));
                    }
                    x = g.x_advance.mul_add(sx, x);
                }
            }
            let argb = u32::from_str_radix(
                text["color_argb"]
                    .as_str()
                    .ok_or("Missing text color")?
                    .trim_start_matches('#'),
                16,
            )
            .map_err(|e| e.to_string())?;
            let color = [
                ((argb >> 16) & 255) as f32 / 255.,
                ((argb >> 8) & 255) as f32 / 255.,
                (argb & 255) as f32 / 255.,
                (argb >> 24) as f32 / 255.,
            ];
            if !vertices.is_empty() {
                out.push(Draw {
                    texture: font.texture.clone(),
                    fill: Fill::TextureClamped,
                    vertices,
                    multiply: std::array::from_fn(|i| {
                        if shadow && i < 3 {
                            0.
                        } else {
                            multiply[i] * color[i]
                        }
                    }),
                    add,
                });
            }
        }
    }
    for child in instance.children.values() {
        visit(movie, vm, shapes, *child, matrix, multiply, add, out)?;
    }
    Ok(())
}
