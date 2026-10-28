use crate::{
    apt_movie::Movie,
    apt_vm::{Host, ObjectKind, Vm},
};
pub use crate::apt_vm::Value;

#[derive(Clone, Copy, Default)]
pub struct Data {
    pub score: f32,
    pub metrics: [f32; 5],
    pub points: [f32; 5],
}

fn array(vm: &mut Vm, values: impl IntoIterator<Item = Value>) -> Result<Value, String> {
    let object = vm.object(ObjectKind::Plain);
    let mut n = 0;
    for value in values {
        vm.set(object, n.to_string(), value)?;
        n += 1;
    }
    vm.set(object, "length", Value::Number(n as f64))?;
    Ok(Value::Object(object))
}

fn score(value: f64) -> String {
    let digits = (value.round() as i64).unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

pub struct Bindings {
    pub movie: Movie,
    pub data: Data,
    pub missing: Vec<String>,
}
pub struct Runtime {
    pub vm: Vm,
    pub bindings: Bindings,
    pub controller: usize,
}

impl Host for Bindings {
    fn property_changed(&mut self, vm: &mut Vm, object: usize, key: &str) -> Result<(), String> {
        if key == "text" || key == "autoSize" {
            self.movie.text_changed(vm, object)?;
        }
        Ok(())
    }
    fn call(&mut self, vm: &mut Vm, object: usize, method: &str, args: Vec<Value>) -> Result<Value, String> {
        if self.movie.method(vm, object, method, &args)? {
            return Ok(Value::Undefined);
        }
        let native = match vm.objects.get(object).map(|o| &o.kind) {
            Some(ObjectKind::Native(name)) => name.clone(),
            _ => "global".into(),
        };
        match (native.as_str(), method) {
            ("Math", "floor") => Ok(Value::Number(args.first().ok_or("Math.floor argument")?.number().floor())),
            ("global", "ASSetPropFlags") => Ok(Value::Undefined),
            ("FELanguage", "GetLocalizedFormatNumberString") => Ok(Value::Text(score(args.first().map_or(0.0, |a| a.number())))),
            ("FELanguage", "GetLocaleText") => {
                let id = args.first().map(|a| a.text()).unwrap_or_default();
                Ok(Value::Text(self.movie.text_assets.language.get(&id).cloned().unwrap_or(id)))
            }
            ("FELanguage", "GetLocalizedFormatStringCustom") => {
                let n = args.get(1).map_or(0.0, |a| a.number());
                Ok(Value::Text(match args.first().map(|a| a.text()).as_deref() {
                    Some("ID_HOM_SPEED_VALUE") => format!("{n:.0} km/h"),
                    Some("ID_COMMON_METRIC_METERS") => format!("{n:.1}m"),
                    _ => format!("{n}"),
                }))
            }
            ("FELanguage", "GetLocalizedTime") => Ok(Value::Text(format!("{:.2}", args.first().map_or(0.0, |a| a.number())))),
            ("HUDComponents", "HOM_GetScore") => Ok(Value::Number(self.data.score.round() as f64)),
            ("HUDComponents", "HOM_GetBonusData") => array(vm, self.data.metrics.map(|v| Value::Number(v as f64))),
            ("HUDComponents", "HOM_GetBonusScores") => array(vm, self.data.points.map(|v| Value::Number(v.round() as f64))),
            ("HUDComponents", "HOM_GetBonusCollisionData" | "HOM_GetBonusCollisionScore") => array(vm, [0.0; 5].map(Value::Number)),
            ("ScreenManager" | "Audio", _) => Ok(Value::Undefined),
            _ => {
                self.missing.push(format!("{native}.{method}({:?})", args));
                Ok(Value::Undefined)
            }
        }
    }
}

impl Runtime {
    pub fn load(json: &serde_json::Value) -> Result<Self, String> {
        let mut vm = Vm::new();
        for name in ["MovieClip", "FELanguage", "HUDComponents", "Math", "ScreenManager", "Audio"] {
            let object = vm.object(ObjectKind::Native(name.into()));
            let prototype = vm.object(ObjectKind::Plain);
            vm.set(object, "prototype", Value::Object(prototype))?;
            vm.set(vm.global, name, Value::Object(object))?;
        }
        vm.set(vm.global, "Screen_EdgeOffset", Value::Number(0.0))?;
        let mut bindings = Bindings { movie: Movie::load(json)?, data: Data::default(), missing: vec![] };
        let initial: Vec<_> = bindings.movie.characters.values().flat_map(|c| &c.frames).flat_map(|f| &f.controls)
            .filter(|c| c.type_name == "do_init_action").map(|c| c.actions_offset).collect();
        for offset in initial {
            let code = bindings.movie.actions[&offset.to_string()].clone();
            vm.run(&code, &mut bindings)?;
        }
        vm.begin_update();
        bindings.movie.initialize(&mut vm)?;
        let mut runtime = Self { vm, bindings, controller: usize::MAX };
        runtime.drain()?;
        let Value::Object(controller) = runtime.vm.get(runtime.bindings.movie.root, "screen") else {
            return Err("Hall of Meat screen: no controller".into());
        };
        runtime.controller = controller;
        Ok(runtime)
    }
    fn drain(&mut self) -> Result<(), String> {
        let mut calls = 0;
        while let Some(crate::apt_movie::Pending { object, offset, .. }) = self.bindings.movie.next_action() {
            calls += 1;
            if calls > 4096 {
                return Err("Hall of Meat frame action limit".into());
            }
            let code = self.bindings.movie.actions.get(&offset.to_string()).ok_or("Missing action block")?.clone();
            self.vm.run_on(object, &code, &mut self.bindings)?;
        }
        Ok(())
    }
    pub fn call(&mut self, name: &str, args: Vec<Value>) -> Result<Value, String> {
        self.vm.begin_update();
        let v = self.vm.call_method(self.controller, name, args, &mut self.bindings)?;
        self.drain()?;
        Ok(v)
    }
    pub fn advance(&mut self) -> Result<(), String> {
        self.vm.begin_update();
        self.bindings.movie.advance(&mut self.vm)?;
        self.drain()?;
        self.vm.collect(self.bindings.movie.instances.keys().copied())?;
        Ok(())
    }
}
