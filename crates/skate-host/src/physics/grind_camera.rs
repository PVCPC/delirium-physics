use crate::camera::CameraGrindOutput;
type V = [f32; 4];

#[derive(Clone, Copy, Debug)]
pub(crate) struct Contact {
    pub point: V,
    pub direction: V,
    pub endpoints: [V; 2],
    pub family: u32,
}

#[derive(Default)]
pub(crate) struct GrindCamera {
    current: V,
    previous: V,
    error: V,
    midpoint: V,
    family: u32,
    active: bool,
}

impl GrindCamera {
    pub fn condition_fields(
        &mut self,
        out: &mut skate_core::player::input_phase::GrindOutputFields,
    ) {
        let vector = |v: [u32; 4]| v.map(f32::from_bits);
        let contact = (out.grinding_316 != 0).then(|| Contact {
            point: vector(out.point_16),
            direction: vector(out.direction_0),
            endpoints: [vector(out.primitive_start_64), vector(out.primitive_end_80)],
            family: out.words_136_140[0],
        });
        let mut camera = CameraGrindOutput {
            direction_0: vector(out.direction_0),
            camera_target_96: vector(out.camera_target_96),
            grinding_316: out.grinding_316,
        };
        self.update(contact, &mut camera);
        out.direction_0 = camera.direction_0.map(f32::to_bits);
        out.camera_target_96 = camera.camera_target_96.map(f32::to_bits);
    }

    pub fn update(&mut self, contact: Option<Contact>, output: &mut CameraGrindOutput) {
        let Some(contact) = contact else {
            self.active = false;
            return;
        };
        let midpoint: V =
            core::array::from_fn(|i| (contact.endpoints[0][i] + contact.endpoints[1][i]) * 0.5);
        if !self.active {
            self.error = [0.0; 4];
            self.current = contact.point;
            self.previous = contact.point;
        } else if contact.family != self.family || midpoint[..3] != self.midpoint[..3] {
            self.error = core::array::from_fn(|i| {
                self.current[i] + (self.current[i] - self.previous[i]) - contact.point[i]
            });
        }
        self.error = self.error.map(|v| v * 0.95);
        self.previous = self.current;
        self.current = core::array::from_fn(|i| contact.point[i] + self.error[i]);
        self.family = contact.family;
        self.midpoint = midpoint;
        self.active = true;
        output.direction_0 = contact.direction;
        output.camera_target_96 = self.current;
    }
}

