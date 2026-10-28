use crate::{
    apt_display::{clip_event, ClipAction, Control, DisplayList, Placement},
    apt_vm::{Instruction, ObjectKind, Value, Vm},
};
use serde::Deserialize;
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Deserialize)]
pub struct Frame {
    pub controls: Vec<Control>,
}
#[derive(Clone, Deserialize)]
pub struct Character {
    pub id: i32,
    pub type_name: String,
    #[serde(default)]
    pub frames: Vec<Frame>,
    pub text: Option<serde_json::Value>,
    pub bounds: Option<[f32; 4]>,
}
#[derive(Clone)]
pub struct Instance {
    pub character: i32,
    pub frame: usize,
    pub playing: bool,
    pub children: BTreeMap<i32, usize>,
    pub placement: Option<Placement>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pending {
    pub object: usize,
    pub offset: u32,
    pub event: u32,
}
#[derive(Clone)]
pub struct Movie {
    pub characters: BTreeMap<i32, Character>,
    pub instances: BTreeMap<usize, Instance>,
    pub actions: BTreeMap<String, Vec<Instruction>>,
    pub pending: VecDeque<Pending>,
    pub immediate: VecDeque<Pending>,
    pub root: usize,
    pub text_assets: crate::apt_text::TextAssets,
    pub shape_origins: BTreeMap<i32, (String, i32)>,
    states: BTreeMap<i32, Vec<DisplayList>>,
}
fn display_states(c: &Character) -> Result<Vec<DisplayList>, String> {
    let mut list = DisplayList::default();
    let mut frames = Vec::new();
    for frame in &c.frames {
        for control in &frame.controls {
            list.apply(control)?;
        }
        frames.push(list.clone());
    }
    Ok(frames)
}
impl Movie {
    pub fn load(json: &serde_json::Value) -> Result<Self, String> {
        let characters: Vec<Character> =
            serde_json::from_value(json["characters"].clone()).map_err(|e| e.to_string())?;
        let mut states = BTreeMap::new();
        for c in &characters {
            states.insert(c.id, display_states(c)?);
        }
        Ok(Self {
            characters: characters.into_iter().map(|c| (c.id, c)).collect(),
            instances: BTreeMap::new(),
            actions: serde_json::from_value(json["actions"].clone()).map_err(|e| e.to_string())?,
            pending: VecDeque::new(),
            immediate: VecDeque::new(),
            root: usize::MAX,
            text_assets: crate::apt_text::TextAssets::load(json)?,
            shape_origins: BTreeMap::new(),
            states,
        })
    }
    pub fn initialize(&mut self, vm: &mut Vm) -> Result<(), String> {
        self.root = self.create(vm, 0, None, 0, &[])?;
        vm.set(self.root, "_root", Value::Object(self.root))?;
        for id in self.instances.keys() {
            vm.set(*id, "_root", Value::Object(self.root))?;
        }
        Ok(())
    }
    fn create(
        &mut self,
        vm: &mut Vm,
        character: i32,
        parent: Option<usize>,
        depth: usize,
        events: &[ClipAction],
    ) -> Result<usize, String> {
        if depth > 32 || self.instances.len() > 2048 {
            return Err("APT movie hierarchy limit".into());
        }
        let c = self
            .characters
            .get(&character)
            .ok_or("APT unknown character")?
            .clone();
        let id = vm.object(ObjectKind::Native(format!("movie:{character}")));
        if self.root != usize::MAX {
            vm.set(id, "_root", Value::Object(self.root))?;
        }
        if let Some(parent) = parent {
            vm.set(id, "_parent", Value::Object(parent))?;
        }
        vm.set(id, "_x", Value::Number(0.0))?;
        vm.set(id, "_y", Value::Number(0.0))?;
        vm.set(id, "_visible", Value::Bool(true))?;
        vm.set(id, "_alpha", Value::Number(100.0))?;
        if let Some(text) = &c.text {
            vm.set(
                id,
                "text",
                Value::Text(text["initial_text"].as_str().unwrap_or("").into()),
            )?;
        }
        self.instances.insert(
            id,
            Instance {
                character,
                frame: 0,
                playing: !c.frames.is_empty(),
                children: BTreeMap::new(),
                placement: None,
            },
        );
        self.text_changed(vm, id)?;
        let sprite = c.type_name == "sprite";
        if sprite {
            self.queue_events(id, events, clip_event::INITIALIZE, true);
            self.queue_events(id, events, clip_event::CONSTRUCT, true);
        }
        if !c.frames.is_empty() {
            self.seek(vm, id, 0, depth + 1)?;
        }
        if sprite {
            self.queue_events(id, events, clip_event::LOAD, false);
        }
        Ok(id)
    }
    pub fn text_changed(&self, vm: &mut Vm, id: usize) -> Result<(), String> {
        let Some(instance) = self.instances.get(&id) else {
            return Ok(());
        };
        let character = &self.characters[&instance.character];
        let Some(text) = &character.text else {
            return Ok(());
        };
        let value = self.text_assets.localize(&vm.get(id, "text").text());
        vm.set(id, "_displayText", Value::Text(value.clone()))?;
        let font = self
            .text_assets
            .fonts
            .get(&(text["font_id"].as_i64().ok_or("Invalid text font id")? as i32))
            .ok_or("Missing original text font")?;
        let height = text["font_height"].as_f64().ok_or("Invalid text size")? as f32;
        let width = font.width(&value, height);
        vm.set(id, "textWidth", Value::Number(width as f64))?;
        let bounds = character.bounds.ok_or("Missing text bounds")?;
        let autosize = vm.get(id, "autoSize").text();
        vm.set(
            id,
            "_width",
            Value::Number(
                if autosize == "left" || autosize == "right" || autosize == "center" {
                    width
                } else {
                    bounds[2] - bounds[0]
                } as f64,
            ),
        )?;
        Ok(())
    }
    fn queue_events(&mut self, object: usize, events: &[ClipAction], event: u32, now: bool) {
        for e in events
            .iter()
            .filter(|e| e.flags & event != 0 && e.actions_offset != 0)
        {
            let p = Pending {
                object,
                offset: e.actions_offset,
                event,
            };
            if now {
                self.immediate.push_back(p);
            } else if event == clip_event::ENTER_FRAME {
                self.pending.push_front(p);
            } else {
                self.pending.push_back(p);
            }
        }
    }
    pub fn next_action(&mut self) -> Option<Pending> {
        loop {
            let p = self
                .immediate
                .pop_front()
                .or_else(|| self.pending.pop_front())?;
            if p.event == clip_event::UNLOAD || self.instances.contains_key(&p.object) {
                return Some(p);
            }
        }
    }
    fn remove(&mut self, vm: &mut Vm, id: usize) {
        if let Some(instance) = self.instances.remove(&id) {
            if let Some(p) = &instance.placement {
                let events = p.clip_actions.clone();
                self.queue_events(id, &events, clip_event::UNLOAD, false);
            }
            for child in instance.children.values() {
                self.remove(vm, *child);
            }
        }
        let _ = vm.set(id, "_visible", Value::Bool(false));
    }
    pub fn seek(
        &mut self,
        vm: &mut Vm,
        id: usize,
        frame: usize,
        nesting: usize,
    ) -> Result<(), String> {
        let instance = self
            .instances
            .get(&id)
            .ok_or("APT absent movie instance")?
            .clone();
        let character = self
            .characters
            .get(&instance.character)
            .ok_or("APT absent movie character")?;
        if frame >= character.frames.len() {
            return Err(format!(
                "APT frame {frame} outside character {}",
                character.id
            ));
        }
        let frame_count = character.frames.len();
        let actions: Vec<_> = character.frames[frame]
            .controls
            .iter()
            .filter(|c| c.type_name == "do_action" && c.actions_offset != 0)
            .map(|c| c.actions_offset)
            .collect();
        let list = self.states[&instance.character][frame].clone();
        let mut children = BTreeMap::new();
        for (depth, placement) in list.depths {
            let previous = instance.children.get(&depth).copied();
            if let Some(name) = previous
                .and_then(|old| self.instances.get(&old))
                .and_then(|i| i.placement.as_ref())
                .map(|p| p.name.clone())
            {
                if !name.is_empty() && name != placement.name {
                    vm.objects[id].fields.remove(&name);
                }
            }
            let child = if let Some(old) = previous.filter(|old| {
                self.instances
                    .get(old)
                    .is_some_and(|i| i.character == placement.character)
            }) {
                old
            } else {
                if let Some(old) = previous {
                    self.remove(vm, old);
                }
                self.create(
                    vm,
                    placement.character,
                    Some(id),
                    nesting + 1,
                    &placement.clip_actions,
                )?
            };
            let old = self.instances[&child].placement.as_ref();
            if old.is_none_or(|old| old.matrix != placement.matrix) {
                vm.set(child, "_x", Value::Number(placement.matrix[4] as f64))?;
                vm.set(child, "_y", Value::Number(placement.matrix[5] as f64))?;
            }
            if !placement.name.is_empty() {
                vm.set(id, &placement.name, Value::Object(child))?;
            }
            self.instances.get_mut(&child).unwrap().placement = Some(placement);
            children.insert(depth, child);
        }
        for (depth, child) in &instance.children {
            if !children.contains_key(depth) {
                if let Some(name) = self
                    .instances
                    .get(child)
                    .and_then(|i| i.placement.as_ref())
                    .map(|p| p.name.clone())
                {
                    if !name.is_empty() {
                        vm.objects[id].fields.remove(&name);
                    }
                }
                self.remove(vm, *child);
            }
        }
        let current = self.instances.get_mut(&id).unwrap();
        current.frame = frame;
        current.children = children;
        vm.set(id, "_currentframe", Value::Number((frame + 1) as f64))?;
        vm.set(id, "_totalframes", Value::Number(frame_count as f64))?;
        for offset in actions {
            self.pending.push_back(Pending {
                object: id,
                offset,
                event: 0,
            });
        }
        Ok(())
    }
    pub fn advance(&mut self, vm: &mut Vm) -> Result<(), String> {
        let ticked: Vec<usize> = self.instances.keys().copied().collect();
        let playing: Vec<_> = self
            .instances
            .iter()
            .filter(|(_, i)| i.playing)
            .map(|(id, i)| (*id, i.character, i.frame))
            .collect();
        for (id, character, frame) in playing {
            if !self.instances.contains_key(&id) {
                continue;
            }
            let count = self.characters[&character].frames.len();
            if count > 1 {
                self.seek(vm, id, (frame + 1) % count, 0)?;
            }
        }
        for id in ticked {
            let Some(instance) = self.instances.get(&id) else {
                continue;
            };
            let Some(events) = instance
                .placement
                .as_ref()
                .filter(|p| !p.clip_actions.is_empty())
                .map(|p| p.clip_actions.clone())
            else {
                continue;
            };
            if self.characters[&instance.character].type_name == "sprite" {
                self.queue_events(id, &events, clip_event::ENTER_FRAME, false);
            }
        }
        Ok(())
    }
    pub fn method(
        &mut self,
        vm: &mut Vm,
        id: usize,
        method: &str,
        args: &[Value],
    ) -> Result<bool, String> {
        if !self.instances.contains_key(&id) {
            return Ok(false);
        }
        match method {
            "stop" => self.instances.get_mut(&id).unwrap().playing = false,
            "play" => self.instances.get_mut(&id).unwrap().playing = true,
            "gotoAndPlay" | "gotoAndStop" => {
                let c = &self.characters[&self.instances[&id].character];
                let frame = match args.first().ok_or("APT goto requires frame")? {
                    Value::Text(label) => c
                        .frames
                        .iter()
                        .position(|f| {
                            f.controls.iter().any(|control| {
                                control.type_name == "frame_label"
                                    && control.label.as_deref() == Some(label)
                            })
                        })
                        .ok_or_else(|| format!("APT character {} lacks label {label}", c.id))?,
                    value => {
                        let frame = value.number();
                        if !frame.is_finite() || frame < 0.0 {
                            return Err(format!(
                                "Invalid APT frame {frame} in {} on character {}",
                                method, c.id
                            ));
                        }
                        (frame as usize).saturating_sub(1)
                    }
                };
                self.seek(vm, id, frame, 0)?;
                self.instances.get_mut(&id).unwrap().playing = method == "gotoAndPlay";
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}

impl Movie {
    pub fn link_imports(
        &mut self,
        key: &str,
        resolution: &crate::apt_imports::Resolution,
        libraries: &BTreeMap<String, Movie>,
    ) -> Result<usize, String> {
        let key = crate::apt_imports::library_key(key);
        let mut memo = BTreeMap::new();
        let mut linked = 0;
        for b in resolution.bindings.iter().filter(|b| b.movie == key) {
            if self.characters.contains_key(&b.character_id) {
                return Err(format!(
                    "APT import {} collides with a local character",
                    b.character_id
                ));
            }
            let library = libraries
                .get(&b.source)
                .ok_or_else(|| format!("APT library {} is not loaded", b.source))?;
            let mut link = Link {
                resolution,
                libraries,
                memo: &mut memo,
            };
            self.copy_character(
                &mut link,
                &b.source,
                library,
                b.source_character,
                Some(b.character_id),
                0,
            )?;
            linked += 1;
        }
        Ok(linked)
    }
    fn fresh_character(&self) -> i32 {
        self.characters.keys().next_back().map_or(1, |k| k + 1)
    }
    fn fresh_action(&self) -> u32 {
        self.actions
            .keys()
            .filter_map(|k| k.parse::<u32>().ok())
            .max()
            .map_or(1, |k| k + 1)
    }
    fn copy_character(
        &mut self,
        link: &mut Link,
        lib_key: &str,
        lib: &Movie,
        source: i32,
        as_id: Option<i32>,
        depth: usize,
    ) -> Result<i32, String> {
        if depth > 64 || self.characters.len() > 65_536 {
            return Err("APT import copy limit".into());
        }
        if as_id.is_none() {
            if let Some(id) = link.memo.get(&(lib_key.to_string(), source)) {
                return Ok(*id);
            }
        }
        let Some(original) = lib.characters.get(&source) else {
            let b = link
                .resolution
                .binding(lib_key, source)
                .ok_or_else(|| format!("APT {lib_key} character {source} is unresolved"))?
                .clone();
            let next = link
                .libraries
                .get(&b.source)
                .ok_or_else(|| format!("APT library {} is not loaded", b.source))?;
            let id =
                self.copy_character(link, &b.source, next, b.source_character, as_id, depth + 1)?;
            link.memo.insert((lib_key.to_string(), source), id);
            return Ok(id);
        };
        let id = as_id.unwrap_or_else(|| self.fresh_character());
        link.memo.entry((lib_key.to_string(), source)).or_insert(id);
        let mut c = original.clone();
        c.id = id;
        if c.type_name == "shape" {
            let origin = lib.shape_origins.get(&source).cloned();
            self.shape_origins
                .insert(id, origin.unwrap_or_else(|| (lib_key.to_string(), source)));
        }
        self.characters.insert(id, c.clone());
        for frame in &mut c.frames {
            for control in &mut frame.controls {
                let placed = matches!(
                    control.type_name.as_str(),
                    "place_object2" | "place_object3"
                ) && control.flags & 2 != 0;
                if let Some(child) = control.character_id.filter(|_| placed) {
                    control.character_id =
                        Some(self.copy_character(link, lib_key, lib, child, None, depth + 1)?);
                }
                for event in &mut control.clip_actions {
                    let code = lib
                        .actions
                        .get(&event.actions_offset.to_string())
                        .ok_or("APT library clip action stream missing")?
                        .clone();
                    let offset = self.fresh_action();
                    self.actions.insert(offset.to_string(), code);
                    event.actions_offset = offset;
                }
                if control.actions_offset != 0
                    && matches!(control.type_name.as_str(), "do_action" | "do_init_action")
                {
                    let code = lib
                        .actions
                        .get(&control.actions_offset.to_string())
                        .ok_or("APT library action stream missing")?
                        .clone();
                    let offset = self.fresh_action();
                    self.actions.insert(offset.to_string(), code);
                    control.actions_offset = offset;
                }
            }
        }
        if let Some(font) = c.text.as_ref().and_then(|t| t["font_id"].as_i64()) {
            let new = self.copy_character(link, lib_key, lib, font as i32, None, depth + 1)?;
            if let Some(text) = &mut c.text {
                text["font_id"] = serde_json::json!(new);
            }
        }
        if let Some(font) = lib.text_assets.fonts.get(&source) {
            self.text_assets.fonts.insert(id, font.clone());
        }
        self.states.insert(id, display_states(&c)?);
        self.characters.insert(id, c);
        Ok(id)
    }
}
struct Link<'a> {
    resolution: &'a crate::apt_imports::Resolution,
    libraries: &'a BTreeMap<String, Movie>,
    memo: &'a mut BTreeMap<(String, i32), i32>,
}

