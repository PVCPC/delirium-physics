mod math;
mod polyline;
mod selection;
pub use selection::best_spline;
use super::Record;
use crate::player::offboard::ground_sync::{Bounds,BoardLimits};
use math::*;
pub use math::closest_point;

pub fn qualify(record: &Record, position: Vector, bounds: Bounds,
    limits: BoardLimits) -> bool {
    let length = record.scalar(176);
    let mut margin = limits.margin;
    if margin > length*0.5 { margin = length*0.5; }
    let distance = polyline::nearest_distance(record,position)
        .max(margin).min(length-margin);
    let point = polyline::at_distance(record,distance);
    let endpoints = record.endpoints();
    let direction = sub(endpoints[0],endpoints[1]);
    let forward = bounds.frame[2];
    let facing = dot(normalize(flatten(forward)),normalize(flatten(direction))).abs();
    let side = normalize(cross([0.,1.,0.,0.],direction));
    let approach = record.vector(96);
    let approach_side = dot(side,approach);
    if approach_side.abs() <= 0.1 || dot(forward,side)*approach_side >= 0.
        || !(facing < 0.8) { return false; }
    let reach = flatten(sub(point,position));
    if !(dot(reach,reach) > 0.) { return false; }
    if !(limits.angle_a > angle(reach,approach.map(|v| -v))) { return false; }
    let mut step = length*0.5;
    if step > f32::from_bits(0x3c23d70a) { step = f32::from_bits(0x3c23d70a); }
    let next_distance = if distance+step > length { distance-step } else { distance+step };
    let tangent = sub(polyline::at_distance(record,next_distance),point);
    let horizontal = flatten(tangent);
    if !(dot(horizontal,horizontal) > f32::from_bits(0x37800000)) { return false; }
    let turns = angle(tangent,horizontal)*f32::from_bits(0x3e22f983);
    let fraction = turns-turns.floor();
    let mut slope = ((fraction-if fraction>0.5 {1.} else {0.})
        *f32::from_bits(0x40c90fdb)).abs();
    if slope > f32::from_bits(0x3fc90fdb) { slope = f32::from_bits(0x40490fdb)-slope; }
    if !(limits.angle_b > slope) { return false; }
    let local = inverse_point(bounds.frame,point);
    (0..3).all(|i| local[i]>=-bounds.extents[i] && local[i]<=bounds.extents[i])
}
