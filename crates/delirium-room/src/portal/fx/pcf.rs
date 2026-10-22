//! `.pcf` particle system definitions on top of the DMX reader.
use std::collections::HashMap;
use std::sync::Arc;

use super::dmx::{self, Attr, Element};

/// One operator / initializer / emitter / renderer / force / constraint.
#[derive(Clone, Debug)]
pub struct OpDef {
    pub function: String,
    pub params: HashMap<String, Attr>,
}

impl OpDef {
    pub fn f(&self, k: &str, default: f32) -> f32 {
        self.params.get(k).and_then(Attr::as_f32).unwrap_or(default)
    }
    pub fn i(&self, k: &str, default: i32) -> i32 {
        self.params.get(k).and_then(Attr::as_i32).unwrap_or(default)
    }
    pub fn b(&self, k: &str, default: bool) -> bool {
        self.params.get(k).and_then(Attr::as_bool).unwrap_or(default)
    }
    pub fn v3(&self, k: &str, default: [f32; 3]) -> [f32; 3] {
        self.params.get(k).and_then(Attr::as_vec3).unwrap_or(default)
    }
    pub fn color(&self, k: &str, default: [u8; 4]) -> [u8; 4] {
        self.params.get(k).and_then(Attr::as_color).unwrap_or(default)
    }
    /// `operator end cap state`: -1 always, 0 only while running, 1 only in the end cap.
    pub fn end_cap_state(&self) -> i32 {
        self.i("operator end cap state", -1)
    }
}

#[derive(Clone, Debug)]
pub struct ChildDef {
    pub name: String,
    pub delay: f32,
    pub end_cap: bool,
}

#[derive(Clone, Debug)]
pub struct SystemDef {
    pub name: String,
    pub material: String,
    pub max_particles: usize,
    pub initial_particles: usize,
    pub radius: f32,
    pub color: [u8; 4],
    pub rotation: f32,
    pub rotation_speed: f32,
    pub sequence_number: i32,
    pub control_point_to_disable_rendering_if_it_is_the_camera: i32,
    pub maximum_time_step: f32,
    pub maximum_sim_tick_rate: f32,
    pub minimum_sim_tick_rate: f32,
    pub view_model_effect: bool,
    pub screen_space_effect: bool,
    pub emitters: Vec<OpDef>,
    pub initializers: Vec<OpDef>,
    pub operators: Vec<OpDef>,
    pub renderers: Vec<OpDef>,
    pub forces: Vec<OpDef>,
    pub constraints: Vec<OpDef>,
    pub children: Vec<ChildDef>,
}

/// All particle systems from a set of `.pcf` files, by name.
#[derive(Default, Debug)]
pub struct ParticleLibrary {
    pub systems: HashMap<String, Arc<SystemDef>>,
}

impl ParticleLibrary {
    pub fn get(&self, name: &str) -> Option<Arc<SystemDef>> {
        self.systems.get(name).cloned()
    }

    /// Add every `DmeParticleSystemDefinition` in a pcf. Later files win.
    pub fn load_pcf(&mut self, data: &[u8]) -> anyhow::Result<usize> {
        let els = dmx::parse(data)?;
        let mut n = 0;
        for e in els.iter().filter(|e| e.ty == "DmeParticleSystemDefinition") {
            let def = system_def(&els, e);
            self.systems.insert(def.name.clone(), Arc::new(def));
            n += 1;
        }
        Ok(n)
    }
}

fn ops(els: &[Element], e: &Element, key: &str) -> Vec<OpDef> {
    e.elements(key)
        .into_iter()
        .filter_map(|i| els.get(i))
        .map(|op| OpDef {
            function: op
                .get("functionName")
                .and_then(Attr::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| op.name.clone()),
            params: op.attrs.clone(),
        })
        .collect()
}

fn system_def(els: &[Element], e: &Element) -> SystemDef {
    let f = |k: &str, d: f32| e.get(k).and_then(Attr::as_f32).unwrap_or(d);
    let i = |k: &str, d: i32| e.get(k).and_then(Attr::as_i32).unwrap_or(d);
    let b = |k: &str, d: bool| e.get(k).and_then(Attr::as_bool).unwrap_or(d);
    let children = e
        .elements("children")
        .into_iter()
        .filter_map(|ci| els.get(ci))
        .filter_map(|c| {
            let def = els.get(c.element("child")?)?;
            Some(ChildDef {
                name: def.name.clone(),
                delay: c.get("delay").and_then(Attr::as_f32).unwrap_or(0.0),
                end_cap: c.get("end cap effect").and_then(Attr::as_bool).unwrap_or(false),
            })
        })
        .collect();
    SystemDef {
        name: e.name.clone(),
        material: e
            .get("material")
            .and_then(Attr::as_str)
            .map(|s| s.replace('\\', "/").to_ascii_lowercase())
            .unwrap_or_default(),
        max_particles: i("max_particles", 1000).max(1) as usize,
        initial_particles: i("initial_particles", 0).max(0) as usize,
        radius: f("radius", 5.0),
        color: e.get("color").and_then(Attr::as_color).unwrap_or([255, 255, 255, 255]),
        rotation: f("rotation", 0.0),
        rotation_speed: f("rotation_speed", 0.0),
        sequence_number: i("sequence_number", 0),
        control_point_to_disable_rendering_if_it_is_the_camera: i(
            "control point to disable rendering if it is the camera",
            -1,
        ),
        maximum_time_step: f("maximum time step", 0.1),
        maximum_sim_tick_rate: f("maximum sim tick rate", 0.0),
        minimum_sim_tick_rate: f("minimum sim tick rate", 0.0),
        view_model_effect: b("view model effect", false),
        screen_space_effect: b("screen space effect", false),
        emitters: ops(els, e, "emitters"),
        initializers: ops(els, e, "initializers"),
        operators: ops(els, e, "operators"),
        renderers: ops(els, e, "renderers"),
        forces: ops(els, e, "forces"),
        constraints: ops(els, e, "constraints"),
        children,
    }
}
