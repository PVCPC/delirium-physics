use super::kv::{self, Kv};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    None,
    NoDraw,
    PlayerClip,
    Clip,
    NonSolid,
    Sky,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendKind {
    Opaque,
    Mask,
    Blend,
    Additive,
}

#[derive(Clone, Debug)]
pub struct MaterialInfo {
    pub name: String,
    pub shader: String,
    pub base_texture: Option<String>,
    pub base_texture2: Option<String>,
    pub blend: BlendKind,
    pub alpha_cutoff: f32,
    pub alpha: f32,
    pub color: [f32; 3],
    pub no_cull: bool,
    pub unlit: bool,
    pub no_portal: bool,
    pub surface_prop: Option<String>,
    pub tool: ToolKind,
    pub decal: bool,
    pub is_glassy: bool,
    pub params: Kv,
}

impl MaterialInfo {
    pub fn missing(name: &str) -> Self {
        let mut m = MaterialInfo {
            name: name.to_string(),
            shader: "missing".into(),
            base_texture: None,
            base_texture2: None,
            blend: BlendKind::Opaque,
            alpha_cutoff: 0.5,
            alpha: 1.0,
            color: [1.0; 3],
            no_cull: false,
            unlit: false,
            no_portal: false,
            surface_prop: None,
            tool: ToolKind::None,
            decal: false,
            is_glassy: false,
            params: Kv::default(),
        };
        m.tool = tool_from_name(name);
        m
    }

    pub fn is_drawn(&self) -> bool {
        self.tool == ToolKind::None
    }
}

pub fn tool_from_name(name: &str) -> ToolKind {
    let n = name.trim_start_matches("materials/");
    let Some(t) = n.strip_prefix("tools/") else {
        return ToolKind::None;
    };
    if t.starts_with("toolsblack") || t.starts_with("toolswhite") {
        return ToolKind::None;
    }
    if t.starts_with("toolsplayerclip") || t.starts_with("toolsnpcclip") {
        ToolKind::PlayerClip
    } else if t.starts_with("toolsclip") {
        ToolKind::Clip
    } else if t.starts_with("toolsskybox") || t.starts_with("toolssky") {
        ToolKind::Sky
    } else if t.starts_with("toolsnodraw")
        || t.starts_with("toolsinvisible")
        || t.starts_with("toolsblocklight")
        || t.starts_with("toolsblockbullets")
        || t.starts_with("toolsblock_los")
        || t.starts_with("toolsblockbright")
    {
        ToolKind::NoDraw
    } else {
        ToolKind::NonSolid
    }
}

pub fn canonical_name(path: &str) -> String {
    let p = super::fs::normalize_path(path);
    let p = p.strip_prefix("materials/").unwrap_or(&p);
    p.strip_suffix(".vmt")
        .or_else(|| p.strip_suffix(".vtf"))
        .unwrap_or(p)
        .to_string()
}

pub fn load_vmt(read: &dyn Fn(&str) -> Option<Vec<u8>>, name: &str) -> Option<(String, Kv)> {
    load_vmt_depth(read, &format!("materials/{}.vmt", canonical_name(name)), 0)
}

fn load_vmt_depth(
    read: &dyn Fn(&str) -> Option<Vec<u8>>,
    path: &str,
    depth: u32,
) -> Option<(String, Kv)> {
    if depth > 8 {
        return None;
    }
    let data = read(path)?;
    let text = String::from_utf8_lossy(&data);
    let kv = kv::parse(&text);
    let (shader, body) = kv.root()?;
    if shader == "patch" {
        let include = body.get("include")?;
        let (base_shader, mut params) = load_vmt_depth(read, &super::fs::normalize_path(include), depth + 1)?;
        for section in ["insert", "replace"] {
            if let Some(b) = body.block(section) {
                for (k, v) in &b.entries {
                    params.set(k, v.clone());
                }
            }
        }
        return Some((base_shader, params));
    }
    Some((shader.to_string(), body.clone()))
}

pub fn texture_param(p: &Kv, key: &str) -> Option<String> {
    let v = p.get(key)?.trim();
    if v.is_empty() || v.starts_with('_') {
        return None;
    }
    Some(canonical_name(v))
}

pub fn material_info(name: &str, shader: &str, p: &Kv) -> MaterialInfo {
    let mut m = MaterialInfo::missing(name);
    m.shader = shader.to_string();
    m.params = p.clone();
    m.base_texture = texture_param(p, "$basetexture");
    m.base_texture2 = texture_param(p, "$basetexture2");
    m.alpha = p.get_f32("$alpha").unwrap_or(1.0);
    m.color = p
        .get("$color")
        .or_else(|| p.get("$color2"))
        .and_then(kv::parse_color)
        .unwrap_or([1.0; 3]);
    m.no_cull = p.get_bool("$nocull");
    m.no_portal = p.get_bool("%noportal");
    m.surface_prop = p.get("$surfaceprop").map(|s| s.to_ascii_lowercase());
    m.decal = p.get_bool("$decal");

    m.blend = if p.get_bool("$additive") {
        BlendKind::Additive
    } else if p.get_bool("$translucent") || m.alpha < 1.0 {
        BlendKind::Blend
    } else if p.get_bool("$alphatest") {
        BlendKind::Mask
    } else {
        BlendKind::Opaque
    };
    m.alpha_cutoff = p.get_f32("$alphatestreference").filter(|v| *v > 0.0).unwrap_or(0.5);

    m.unlit = matches!(
        shader,
        "unlitgeneric" | "unlittwotexture" | "sprite" | "spritecard" | "splinerope" | "cable" | "monitorscreen"
    );

    m.tool = if p.get_bool("%compilesky") || p.get_bool("%compile2dsky") {
        ToolKind::Sky
    } else if p.get_bool("%compileplayerclip") || p.get_bool("%compilenpcclip") {
        ToolKind::PlayerClip
    } else if p.get_bool("%compileclip") {
        ToolKind::Clip
    } else if p.get_bool("%compilenodraw") || p.get_bool("%compileinvisible") {
        ToolKind::NoDraw
    } else if p.get_bool("%compiletrigger")
        || p.get_bool("%compilehint")
        || p.get_bool("%compileskip")
        || p.get_bool("%compileorigin")
        || p.get_bool("%compileareaportal")
        || p.get_bool("%compileoccluder")
        || p.get_bool("%compilefog")
    {
        ToolKind::NonSolid
    } else {
        tool_from_name(name)
    };

    m.is_glassy = matches!(shader, "water" | "refract" | "lightmappedreflective" | "portalrefract");
    if m.is_glassy && m.blend == BlendKind::Opaque {
        m.blend = BlendKind::Blend;
    }
    m
}

pub fn resolve(read: &dyn Fn(&str) -> Option<Vec<u8>>, name: &str) -> MaterialInfo {
    let canon = canonical_name(name);
    match load_vmt(read, &canon) {
        Some((shader, params)) => material_info(&canon, &shader, &params),
        None => MaterialInfo::missing(&canon),
    }
}

