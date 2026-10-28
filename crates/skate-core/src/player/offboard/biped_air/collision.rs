use super::{DT, Frame, State, Vector, math::*};
use crate::player::offboard::air_launch::Packet;

#[derive(Clone, Copy, Debug)]
pub struct Response {
    pub request_52: bool,
    pub restart: Option<Vector>,
}

fn remove_positive(value: Vector, direction: Vector) -> Vector {
    let component = dot(value, normalize_or(direction, ZERO));
    if component > 0. {
        sub(value, scale(direction, component))
    } else {
        value
    }
}

impl State {
    pub fn collision_response(
        &mut self,
        errors_16288_16304: [Vector; 2],
        up_544: Vector,
        restart_allowed_8493: bool,
    ) -> Response {
        let vertical = errors_16288_16304.map(|v| dot(v, up_544).abs());
        let residual = std::array::from_fn::<_, 2, _>(|i| {
            length(sub(
                errors_16288_16304[i],
                scale(up_544, vertical[i]).map(f32::abs),
            ))
        });
        let request_52 = vertical.iter().chain(&residual).any(|&v| v > 0.2);
        let planar = errors_16288_16304.map(|v| [v[0], 0., v[2], v[0]]);
        let sizes = planar.map(length);
        let mut response = Response {
            request_52,
            restart: None,
        };
        if !(sizes[0] > 0.01 || sizes[1] > 0.01) {
            return response;
        }
        if self.flags_544_550[6] {
            self.flags_544_550[3] = true;
            return response;
        }
        let normal = normalize(if sizes[0] > sizes[1] {
            planar[0]
        } else {
            planar[1]
        });
        let velocity = self.result.velocity_288;
        if (sizes[0] > 0.01 || sizes[1] > 0.01) && dot(normal, velocity) < -10. {
            self.flags_544_550[3] = true;
        }
        if !(dot(velocity, normal) < 0.) {
            return response;
        }
        let above_root =
            self.result.scalar_392 > self.frame_208[3][1] && self.result.normal_304[1] > 0.7;
        if restart_allowed_8493 && !above_root {
            response.restart = Some(normal);
        }
        response
    }

    pub fn restart_packet(
        &mut self,
        mut packet: Packet,
        normal: Vector,
        position_592: Vector,
        up_544: Vector,
        foot_height: f32,
    ) -> Packet {
        self.flags_544_550[2] = true;
        let horizontal = normal;
        let first = remove_positive(self.result.velocity_288, scale(horizontal, -1.));
        let second = remove_positive(first, scale(normal, -1.));
        packet.velocity_0 = madd(normal, 1., second);
        packet.board_position_80 = madd(up_544, foot_height, position_592);
        packet.position_32 = madd(packet.velocity_0, DT, position_592);
        packet.has_board_position_116 = true;
        self.restart_normal_528 = normal;
        packet
    }

    pub fn finish_restart(&mut self, effective_animation_frame: Frame, normal: Vector) {
        self.frame_452 = 0;
        self.flags_544_550[0] = false;
        self.flags_544_550[1] = false;
        self.frame_80 = effective_animation_frame;
        self.initial_up_480 = normal;
    }

    pub fn correct_restarted_sample(&mut self) {
        if self.flags_544_550[2] {
            let step = scale(self.result.velocity_288, DT);
            self.result.position_272 = add(
                sub(self.result.position_272, step),
                remove_positive(step, scale(self.restart_normal_528, -1.)),
            );
        }
    }
}
