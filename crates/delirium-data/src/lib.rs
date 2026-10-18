pub static LE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn u32_of(b: [u8; 4]) -> u32 {
    if LE.load(std::sync::atomic::Ordering::Relaxed) { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) }
}

pub(crate) fn u16_of(b: [u8; 2]) -> u16 {
    if LE.load(std::sync::atomic::Ordering::Relaxed) { u16::from_le_bytes(b) } else { u16::from_be_bytes(b) }
}
pub mod actiontable;
pub mod anim;
pub mod animblend;
pub mod animgrp;
pub mod archive_key;
pub mod bounds;
pub mod carscan;
pub mod controls;
pub mod crt;
pub mod csv;
pub mod damage;
pub mod disc;
pub mod estimate;
pub mod fall;
pub mod follow;
pub mod getup;
pub mod ide;
pub mod img;
pub mod ipltext;
pub mod lodtree;
pub mod map;
pub mod mathrt;
pub mod melee;
pub mod melee_task;
pub mod meleeanims;
pub mod model;
pub mod movement;
pub mod nm_asset;
pub mod pad;
pub mod ped;
pub mod pedgroups;
pub mod pedpersonality;
pub mod ragdoll;
pub mod resource;
pub mod rng;
pub mod route;
pub mod rpf;
pub mod runanim;
pub mod sce;
pub mod skeleton;
pub mod staticcount;
pub mod stfs;
pub mod suspension;
pub mod taskparams;
pub mod tasks;
pub mod textfmt;
pub mod textlines;
pub mod transmission;
pub mod tyre;
pub mod vehicles;
pub mod weapons;
pub mod wheelstep;
pub mod xex;
