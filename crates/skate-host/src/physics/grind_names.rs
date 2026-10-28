#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Name {
    pub skating_id: i32,
    pub attribute: &'static str,
    pub display: &'static str,
    pub scorable_id: i32,
}
#[path = "grind_names/table0.rs"]
mod table0;
#[path = "grind_names/table1.rs"]
mod table1;
#[path = "grind_names/table2.rs"]
mod table2;
#[path = "grind_names/table3.rs"]
mod table3;
#[path = "grind_names/table4.rs"]
mod table4;
#[path = "grind_names/table5.rs"]
mod table5;
#[path = "grind_names/table6.rs"]
mod table6;
#[path = "grind_names/table7.rs"]
mod table7;

pub(crate) fn lookup(c: [u32; 6]) -> Option<Name> {
    if c.into_iter()
        .zip([2, 2, 2, 2, 4, 6])
        .any(|(v, bound)| v >= bound)
    {
        return None;
    }
    let i = (((((c[0] * 2 + c[1]) * 2 + c[2]) * 2 + c[3]) * 4 + c[4]) * 6 + c[5]) as usize;
    let table = match i / 48 {
        0 => &table0::TABLE,
        1 => &table1::TABLE,
        2 => &table2::TABLE,
        3 => &table3::TABLE,
        4 => &table4::TABLE,
        5 => &table5::TABLE,
        6 => &table6::TABLE,
        _ => &table7::TABLE,
    };
    Some(table[i % 48])
}
