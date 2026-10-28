use super::{
    contact_queries::Input,
    contact_records::{Direction, Records, Source},
};

pub fn promote(input: Input, records: &mut Records) {
    let original_count = records.surface.len();
    for next in 1..original_count {
        let previous = records.surface[next - 1];
        let current = records.surface[next];
        let a = previous.coordinates;
        let b = current.coordinates;
        let mut low = if b[1] > a[1] { a[1] } else { b[1] };
        let mut high = if a[1] > b[1] { a[1] } else { b[1] };
        let mut index = 0;
        while index < records.obstacle.len() {
            let obstacle = records.obstacle[index];
            let p = obstacle.coordinates;
            if a[0] > p[0] || p[0] > b[0] {
                index += 1;
                continue;
            }
            if low > p[1] {
                records.obstacle[index].flags |= 2;
                index += 1;
                continue;
            }
            if high > p[1]
                || obstacle.distance < previous.distance
                || obstacle.distance > current.distance
            {
                index += 1;
                continue;
            }
            records.obstacle[index].flags |= 2;
            let mut lower = index;
            let mut upper = index;
            index += 1;
            while index < records.obstacle.len() {
                let p = records.obstacle[index].coordinates;
                if (records.obstacle[lower].coordinates[0] - p[0]).abs()
                    > f32::from_bits(0x3a83_126f)
                {
                    break;
                }
                records.obstacle[index].flags |= 2;
                if p[1] >= high && records.obstacle[lower].coordinates[1] > p[1] {
                    lower = index;
                }
                if p[1] > records.obstacle[upper].coordinates[1] {
                    upper = index;
                }
                index += 1;
            }
            records.obstacle[lower].flags = (records.obstacle[lower].flags & !2) | 1;
            records.obstacle[upper].flags = (records.obstacle[upper].flags & !2) | 1;
            let mut insert = |index: usize, low: &mut f32, high: &mut f32| {
                let r = records.obstacle[index];
                let mut normal = r.normal;
                records.insert(
                    input,
                    r.position,
                    &mut normal,
                    Source::Support,
                    Direction::Forward,
                    r.distance,
                );
                if r.coordinates[1] > *high {
                    *low = *high;
                    *high = r.coordinates[1];
                } else if r.coordinates[1] > *low {
                    *low = r.coordinates[1];
                }
            };
            insert(lower, &mut low, &mut high);
            if upper != lower {
                insert(upper, &mut low, &mut high);
            }
            index += 1;
        }
    }
    super::contact_sort::sort(&mut records.surface);
}

