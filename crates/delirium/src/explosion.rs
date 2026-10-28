use crate::msg::Value;
use crate::shottask::Msg;
use delirium_data::rng::Rand;
use delirium_data::taskparams::TaskParams;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Set {
    pub f: [f32; 27],
    pub pedal: [i32; 2],
    pub catch: [f32; 2],
}

const NAMES: [&str; 27] = ["windmillArmStiffness", "windmillBodyStiffness", "windmillPeriode", "windmillAmplitude",
    "windmillPhase", "pedalLegStiffness", "pedalRadius", "pedalSpeed", "pedalOffset", "pedalAsymmetry",
    "writheArmStiffness", "writheLegStiffness", "writheBackStiffness", "writheLegPeriod", "writheArmPeriod",
    "writheBackPeriod", "writheDamping", "writheLegAmplitude", "writheArmAmplitude", "writheBackAmplitude",
    "writheKneeAmplitude", "writheElbowAmplitude", "rollupStiffness", "rollupLegPush", "rollupArmReachAmount",
    "rollupArmToSlowDown", "rollupAsymmetricalLegs"];

impl Set {
    pub fn read(tp: &TaskParams) -> Set {
        let e = |n| tp.get("NMExplosion", "*", n, 0.0);
        let j = |n| tp.get("NMJumpRollFromRoadVehicle", "*", n, 0.0);
        Set { f: NAMES.map(e), pedal: [e("minPedalTimeInRollUp") as i32, e("maxPedalTimeInRollUp") as i32],
              catch: [j("catchFallBodyStiffness"), j("zAxisSpinReduction")] }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Explosion {
    pub started: Option<u32>,
    pub state: u32,
    pedal: i32,
    catch: bool,
}

fn coin(rand: &mut Rand) -> bool {
    rand.next() < delirium_data::kv!(46 int)
}

fn pedal_legs(set: &Set, back: bool) -> Msg {
    use Value::{Bool as B, Float as F};
    let f = &set.f;
    ("pedalLegs", vec![("start", B(true)), ("backPedal", B(back)), ("legStiffness", F(f[delirium_data::kv!(2 int)])), ("pedalLeftLeg", B(true)),
                       ("pedalRightLeg", B(true)), ("radius", F(f[delirium_data::kv!(6 int)])), ("angularSpeed", F(f[delirium_data::kv!(22 int)])), ("pedalOffset", F(f[delirium_data::kv!(23 int)])),
                       ("speedAsymmetry", F(f[delirium_data::kv!(97 int)])), ("adaptivePedal4Dragging", B(false))])
}

impl Explosion {
    pub fn update(&mut self, set: &Set, rand: &mut Rand, now: u32, vz: f32, near: bool) -> Vec<Msg> {
        let mut m = vec![];
        match self.started {
            None => {
                self.started = Some(now);
                m.push(("stopAllBehaviours", vec![]));
                self.start(set, rand, &mut m);
            }
            Some(t0) => self.control(set, now, t0, vz, near, &mut m),
        }
        m
    }

    fn start(&mut self, set: &Set, rand: &mut Rand, m: &mut Vec<Msg>) {
        use Value::{Bool as B, Float as F};
        let f = &set.f;
        let (roll, writhe) = (coin(rand), coin(rand));
        self.catch = coin(rand);
        self.pedal = rand.int_in_range(set.pedal[0], set.pedal[1]);
        let fb = || Value::Str("fb".into());
        if roll {
            m.push(pedal_legs(set, coin(rand)));
            m.push(("bodyRollUp", vec![("start", B(true)), ("mask", fb()), ("stiffness", F(f[delirium_data::kv!(98 int)])), ("legPush", F(f[delirium_data::kv!(99 int)])),
                                       ("armReachAmount", F(f[delirium_data::kv!(100 int)])), ("useArmToSlowDown", F(f[delirium_data::kv!(101 int)])), ("asymmetricalLegs", F(f[delirium_data::kv!(102 int)]))]));
            self.state = 2;
        } else if writhe {
            let mut w = vec![("start", B(true))];
            w.extend(["armStiffness", "legStiffness", "backStiffness", "legPeriod", "armPeriod", "backPeriod"].into_iter().zip(&f[delirium_data::kv!(77 int)..delirium_data::kv!(51 int)]).map(|(k, x)| (k, F(*x))));
            w.extend(["legDamping", "armDamping", "backDamping"].map(|k| (k, F(f[delirium_data::kv!(51 int)]))));
            w.extend(["legAmplitude", "armAmplitude", "backAmplitude", "kneeAmplitude", "elbowAmplitude"].into_iter().zip(&f[delirium_data::kv!(103 int)..delirium_data::kv!(98 int)]).map(|(k, x)| (k, F(*x))));
            w.extend([("mask", fb()), ("rollOverFlag", B(false))]);
            m.push(("bodyWrithe", w));
            self.state = 1;
        } else {
            m.push(("armsWindmillAdaptive", vec![("start", B(true)), ("armStiffness", F(f[0])), ("bodyStiffness", F(f[1])),
                                                 ("period", F(f[2])), ("amplitude", F(f[delirium_data::kv!(0 int)])), ("phase", F(f[delirium_data::kv!(1 int)])),
                                                 ("disableOnImpact", B(true))]));
            m.push(pedal_legs(set, coin(rand)));
            self.state = 0;
        }
    }

    fn control(&mut self, set: &Set, now: u32, t0: u32, vz: f32, near: bool, m: &mut Vec<Msg>) {
        use Value::{Bool as B, Float as F};
        if self.state < delirium_data::kv!(0 int) && vz < -delirium_data::kv!(61) && near {
            if self.catch {
                let [k, spin] = set.catch;
                m.push(("stopAllBehaviours", vec![]));
                m.push(("catchFall", vec![("start", B(true)), ("legsStiffness", F(k)), ("armsStiffness", F(k)), ("torsoStiffness", F(k)),
                                          ("zAxisSpinReduction", F(spin))]));
                self.state = delirium_data::kv!(1 int);
            } else {
                m.push(("pedalLegs", vec![("start", B(false))]));
                self.state = delirium_data::kv!(0 int);
            }
        }
        if self.state == 2 && self.pedal > 0 && now > t0.wrapping_add(self.pedal as u32) {
            m.push(("pedalLegs", vec![("start", B(false))]));
            self.state = delirium_data::kv!(0 int);
        }
    }
}

