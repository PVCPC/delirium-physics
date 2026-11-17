pub type Vec3 = [f32; 3];
pub type Corners = [Vec3; 4];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RoomId(pub u32);

impl RoomId {
    pub const SHELL: RoomId = RoomId(0);
    pub const OUTSIDE: RoomId = RoomId(u32::MAX);

    pub fn is_room(self) -> bool {
        self.0 as i32 > 0
    }
    pub fn index(self) -> usize {
        self.0 as usize
    }
    pub fn from_index(n: usize) -> RoomId {
        RoomId(n as u32)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PortalId(pub u32);

impl PortalId {
    pub const NONE: PortalId = PortalId(u32::MAX);
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShellPortal(pub u32);

impl ShellPortal {
    pub const NONE: ShellPortal = ShellPortal(u32::MAX);
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RoomKey(pub u32);

impl RoomKey {
    pub const NONE: RoomKey = RoomKey(0);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InteriorHandle(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntityType {
    Building,
    Vehicle,
    Ped,
    Object,
    DummyPed,
    Portal,
    Interior,
    Other(u8),
}

impl EntityType {
    pub fn from_number(n: u32) -> EntityType {
        match n {
            1 => EntityType::Building,
            2 => EntityType::Vehicle,
            3 => EntityType::Ped,
            4 => EntityType::Object,
            6 => EntityType::DummyPed,
            7 => EntityType::Portal,
            8 => EntityType::Interior,
            k => EntityType::Other(k as u8),
        }
    }
    pub fn number(self) -> u32 {
        match self {
            EntityType::Building => 1,
            EntityType::Vehicle => 2,
            EntityType::Ped => 3,
            EntityType::Object => 4,
            EntityType::DummyPed => 6,
            EntityType::Portal => 7,
            EntityType::Interior => 8,
            EntityType::Other(k) => k as u32,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntityStatus {
    Player,
    Ordinary,
    Wrecked,
    Other(u8),
}

impl EntityStatus {
    pub fn from_number(n: u32) -> EntityStatus {
        match n {
            0 => EntityStatus::Player,
            2 => EntityStatus::Ordinary,
            3 => EntityStatus::Wrecked,
            k => EntityStatus::Other(k as u8),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntityFlag {
    Inside,
    ReflectionClip,
    LostInterior,
    NoRetry,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VehicleState {
    Unknown3,
    Other(u32),
}

impl VehicleState {
    pub fn from_number(n: u32) -> VehicleState {
        match n {
            3 => VehicleState::Unknown3,
            k => VehicleState::Other(k),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomState {
    Left,
    Entered,
}

pub fn signed_count(n: usize) -> usize {
    (n as u32 as i32).max(0) as usize
}

pub trait TrackerRooms {
    type Tracker: Copy + PartialEq;
    type Entity: Copy + PartialEq;
    type Interior: Copy + PartialEq;
    type Mlo: Copy;
    type PortalDef: Copy;
    type PortalObject: Copy;
    type List: Copy;
    type Node: Copy + PartialEq;
    type NodePool: Copy;
    type Item: Copy + PartialEq;

    fn tracker_item(&self, t: Self::Tracker) -> Self::Item;
    fn interior_item(&self, i: Self::Interior) -> Self::Item;
    fn node_pool(&mut self) -> Self::NodePool;
    fn take_node(&mut self, pool: Self::NodePool) -> Option<Self::Node>;
    fn free_node(&mut self, pool: Self::NodePool, node: Self::Node);
    fn list_head(&mut self, list: Self::List) -> Option<Self::Node>;
    fn set_list_head(&mut self, list: Self::List, node: Option<Self::Node>);
    fn node_item(&mut self, node: Self::Node) -> Self::Item;
    fn set_node_item(&mut self, node: Self::Node, item: Self::Item);
    fn node_next(&mut self, node: Self::Node) -> Option<Self::Node>;
    fn set_node_next(&mut self, node: Option<Self::Node>, next: Option<Self::Node>);
    fn readied_interiors(&mut self) -> Self::List;
    fn shown_interiors(&mut self) -> Self::List;
    fn watched_trackers(&mut self) -> Self::List;

    fn tracker_interior(&mut self, t: Self::Tracker) -> Option<Self::Interior>;
    fn set_tracker_interior(&mut self, t: Self::Tracker, interior: Option<Self::Interior>);
    fn tracker_entity(&mut self, t: Self::Tracker) -> Option<Self::Entity>;
    fn tracker_room(&mut self, t: Self::Tracker) -> RoomId;
    fn set_tracker_room(&mut self, t: Self::Tracker, room: RoomId);
    fn tracker_listed_watched(&mut self, t: Self::Tracker) -> bool;
    fn set_tracker_listed_watched(&mut self, t: Self::Tracker, listed: bool);
    fn tracker_watched_state(&mut self, t: Self::Tracker) -> u8;
    fn set_tracker_watched_state(&mut self, t: Self::Tracker, state: u8);
    fn is_camera_tracker(&mut self, t: Self::Tracker) -> bool;

    fn room_record_capacity(&mut self, i: Self::Interior) -> usize;
    fn room_tracker_list(&mut self, i: Self::Interior, room: RoomId) -> Self::List;
    fn room_entity_count(&mut self, i: Self::Interior, room: RoomId) -> usize;
    fn room_entity(&mut self, i: Self::Interior, room: RoomId, n: usize) -> Option<Self::Entity>;
    fn mark_interior_touched(&mut self, i: Self::Interior);
    fn unknown_touched_interior_step(&mut self, i: Self::Interior);
    fn portal_record_corners(&mut self, i: Self::Interior, n: PortalId) -> Corners;
    fn interior_handle(&mut self, i: Self::Interior) -> InteriorHandle;
    fn interior_portal_objects_made(&mut self, i: Self::Interior) -> bool;
    fn interior_portal_object(&mut self, i: Self::Interior, k: usize) -> Option<Self::PortalObject>;

    fn interior_mlo(&mut self, i: Self::Interior) -> Self::Mlo;
    fn mlo_room_count(&mut self, mlo: Self::Mlo) -> usize;
    fn mlo_room_portal_count(&mut self, mlo: Self::Mlo, room: RoomId) -> usize;
    fn mlo_room_key(&mut self, mlo: Self::Mlo, room: RoomId) -> RoomKey;
    fn room_portal_number(&mut self, mlo: Self::Mlo, room: RoomId, k: usize) -> PortalId;
    fn portal_def(&mut self, mlo: Self::Mlo, n: PortalId) -> Self::PortalDef;
    fn portal_def_is_link(&mut self, def: Self::PortalDef) -> bool;
    fn portal_def_rooms(&mut self, def: Self::PortalDef) -> (RoomId, RoomId);
    fn portal_def_facing_room(&mut self, def: Self::PortalDef) -> RoomId;

    fn portal_object_side(&mut self, p: Self::PortalObject) -> (Option<Self::Interior>, ShellPortal);
    fn portal_object_other_side(&mut self, p: Self::PortalObject) -> (Option<Self::Interior>, ShellPortal);

    fn entity_flag(&mut self, e: Self::Entity, flag: EntityFlag) -> bool;
    fn set_entity_flag(&mut self, e: Self::Entity, flag: EntityFlag, on: bool);
    fn entity_type(&mut self, e: Self::Entity) -> EntityType;
    fn entity_status(&mut self, e: Self::Entity) -> EntityStatus;
    fn entity_made_for_mission(&mut self, e: Self::Entity) -> bool;
    fn set_entity_room(&mut self, e: Self::Entity, room: RoomId);
    fn set_entity_room_state(&mut self, e: Self::Entity, state: RoomState);
    fn set_entity_interior_handle(&mut self, e: Self::Entity, handle: Option<InteriorHandle>);
    fn set_entity_lost_interior_2(&mut self, e: Self::Entity);
    fn entity_lost_interior(&mut self, e: Self::Entity);
    fn entity_went_inside(&mut self, e: Self::Entity);
    fn entity_went_outside(&mut self, e: Self::Entity);

    fn player_ped(&mut self) -> Option<Self::Entity>;
    fn player_ped_outside(&mut self, ped: Self::Entity);
}

pub fn list_has<G: TrackerRooms>(g: &mut G, list: G::List, item: G::Item) -> bool {
    let mut node = g.list_head(list);
    while let Some(n) = node {
        if g.node_item(n) == item {
            return true;
        }
        node = g.node_next(n);
    }
    false
}

pub fn list_push<G: TrackerRooms>(g: &mut G, list: G::List, item: G::Item) {
    let pool = g.node_pool();
    let node = g.take_node(pool);
    if let Some(node) = node {
        g.set_node_item(node, item);
    }
    let head = g.list_head(list);
    g.set_node_next(node, head);
    g.set_list_head(list, node);
}

pub fn list_remove<G: TrackerRooms>(g: &mut G, list: G::List, item: G::Item) {
    let first = g.list_head(list);
    let (mut before, mut node) = (None, first);
    let found = loop {
        let Some(n) = node else { return };
        if g.node_item(n) == item {
            break n;
        }
        before = Some(n);
        node = g.node_next(n);
    };
    if first == Some(found) {
        let next = g.node_next(found);
        g.set_list_head(list, next);
    } else if before.is_some() {
        let next = g.node_next(found);
        g.set_node_next(before, next);
    }
    let pool = g.node_pool();
    g.free_node(pool, found);
}

pub fn note_readied<G: TrackerRooms>(g: &mut G, interior: G::Interior) {
    let list = g.readied_interiors();
    let item = g.interior_item(interior);
    if !list_has(g, list, item) {
        list_push(g, list, item);
    }
}

pub fn show_interior<G: TrackerRooms>(g: &mut G, interior: G::Interior) {
    let list = g.shown_interiors();
    let item = g.interior_item(interior);
    if !list_has(g, list, item) {
        list_push(g, list, item);
    }
}

pub fn hide_interior<G: TrackerRooms>(g: &mut G, interior: G::Interior) {
    let list = g.shown_interiors();
    let item = g.interior_item(interior);
    if list_has(g, list, item) {
        list_remove(g, list, item);
    }
}

pub fn room_count<G: TrackerRooms>(g: &mut G, interior: G::Interior) -> usize {
    let mlo = g.interior_mlo(interior);
    g.mlo_room_count(mlo)
}

pub fn room_portal_count<G: TrackerRooms>(g: &mut G, interior: G::Interior, room: RoomId) -> usize {
    let mlo = g.interior_mlo(interior);
    if room.index() < g.mlo_room_count(mlo) {
        g.mlo_room_portal_count(mlo, room)
    } else {
        0
    }
}

pub fn room_key<G: TrackerRooms>(g: &mut G, interior: G::Interior, room: RoomId) -> RoomKey {
    let mlo = g.interior_mlo(interior);
    g.mlo_room_key(mlo, room)
}

pub fn portal_number<G: TrackerRooms>(g: &mut G, interior: G::Interior, room: RoomId, k: usize) -> PortalId {
    let mlo = g.interior_mlo(interior);
    g.room_portal_number(mlo, room, k)
}

pub fn room_beyond<G: TrackerRooms>(g: &mut G, interior: G::Interior, room: RoomId, k: usize) -> RoomId {
    let mlo = g.interior_mlo(interior);
    let n = g.room_portal_number(mlo, room, k);
    let def = g.portal_def(mlo, n);
    let (a, b) = g.portal_def_rooms(def);
    if room == a {
        b
    } else if room == b {
        a
    } else {
        RoomId::SHELL
    }
}

pub fn portal_object<G: TrackerRooms>(g: &mut G, interior: G::Interior, room: RoomId, k: usize) -> Option<G::PortalObject> {
    if !g.interior_portal_objects_made(interior) {
        return None;
    }
    if room == RoomId::SHELL {
        return g.interior_portal_object(interior, k);
    }
    let want = portal_number(g, interior, room, k);
    let mlo = g.interior_mlo(interior);
    if g.mlo_room_count(mlo) == 0 {
        return None;
    }
    let outside_portals = g.mlo_room_portal_count(mlo, RoomId::SHELL);
    let j = (0..outside_portals).find(|&j| g.room_portal_number(mlo, RoomId::SHELL, j) == want)?;
    g.interior_portal_object(interior, j)
}

pub fn portal_is_link<G: TrackerRooms>(g: &mut G, interior: G::Interior, room: RoomId, k: usize) -> bool {
    let mlo = g.interior_mlo(interior);
    let n = g.room_portal_number(mlo, room, k);
    let def = g.portal_def(mlo, n);
    g.portal_def_is_link(def)
}

pub fn portal_corners<G: TrackerRooms>(g: &mut G, interior: G::Interior, room: RoomId, k: usize) -> Corners {
    let mlo = g.interior_mlo(interior);
    let n = g.room_portal_number(mlo, room, k);
    let corners = g.portal_record_corners(interior, n);
    let def = g.portal_def(mlo, n);
    if room == g.portal_def_facing_room(def) {
        corners
    } else {
        [corners[2], corners[1], corners[0], corners[3]]
    }
}

pub fn portal_other_side<G: TrackerRooms>(
    g: &mut G,
    p: G::PortalObject,
    interior: Option<G::Interior>,
    was: (Option<G::Interior>, ShellPortal),
) -> (Option<G::Interior>, ShellPortal) {
    let other = g.portal_object_other_side(p);
    let own = g.portal_object_side(p);
    if interior == other.0 {
        own
    } else if interior == own.0 {
        other
    } else {
        was
    }
}

pub fn tracker_leave_room<G: TrackerRooms>(g: &mut G, interior: G::Interior, t: G::Tracker) {
    let room = g.tracker_room(t);
    let item = g.tracker_item(t);
    if g.room_record_capacity(interior) as i32 > room.0 as i32 {
        let list = g.room_tracker_list(interior, room);
        if list_has(g, list, item) {
            list_remove(g, list, item);
            if let Some(e) = g.tracker_entity(t) {
                g.set_entity_flag(e, EntityFlag::Inside, false);
                g.set_entity_room_state(e, RoomState::Left);
            }
        }
    }
}

pub fn tracker_enter_room<G: TrackerRooms>(g: &mut G, interior: G::Interior, t: G::Tracker) {
    let room = g.tracker_room(t);
    if !room.is_room() {
        return;
    }
    let list = g.room_tracker_list(interior, room);
    let item = g.tracker_item(t);
    if list_has(g, list, item) {
        return;
    }
    list_push(g, list, item);
    if let Some(e) = g.tracker_entity(t) {
        g.set_entity_flag(e, EntityFlag::ReflectionClip, false);
        g.set_entity_flag(e, EntityFlag::Inside, true);
        g.set_entity_room_state(e, RoomState::Entered);
    }
}

pub fn interior_touched<G: TrackerRooms>(g: &mut G, interior: G::Interior) {
    g.mark_interior_touched(interior);
    g.unknown_touched_interior_step(interior);
}

pub fn tracker_player_outside<G: TrackerRooms>(g: &mut G, t: G::Tracker) {
    let Some(e) = g.tracker_entity(t) else { return };
    if g.entity_type(e) != EntityType::Ped {
        return;
    }
    if g.player_ped() == Some(e) {
        g.player_ped_outside(e);
    }
}

pub fn tracker_set_room<G: TrackerRooms>(g: &mut G, t: G::Tracker, room: RoomId) {
    let Some(interior) = g.tracker_interior(t) else { return };
    if g.tracker_room(t).is_room() {
        tracker_leave_room(g, interior, t);
    }
    g.set_tracker_room(t, room);
    if room.is_room() {
        tracker_enter_room(g, interior, t);
    }
    if let Some(e) = g.tracker_entity(t) {
        g.set_entity_room(e, room);
    }
}

pub fn tracker_leave_interior<G: TrackerRooms>(g: &mut G, t: G::Tracker) {
    if let Some(interior) = g.tracker_interior(t) {
        if g.tracker_room(t).is_room() {
            tracker_leave_room(g, interior, t);
            g.set_tracker_room(t, RoomId::SHELL);
        }
        g.set_tracker_interior(t, None);
    }
    if let Some(e) = g.tracker_entity(t) {
        g.set_entity_room(e, RoomId::OUTSIDE);
        g.set_entity_interior_handle(e, None);
    }
}

pub fn tracker_set_interior<G: TrackerRooms>(g: &mut G, t: G::Tracker, interior: Option<G::Interior>, room: RoomId) {
    let e = g.tracker_entity(t);
    let (was_interior, was_room) = (g.tracker_interior(t), g.tracker_room(t));
    let inside = if g.is_camera_tracker(t) {
        was_interior.is_some() && was_room.is_room()
    } else {
        e.is_some_and(|e| g.entity_flag(e, EntityFlag::Inside))
    };
    match interior {
        Some(interior) if room.is_room() => {
            if !inside {
                if let Some(e) = e {
                    g.entity_went_inside(e);
                }
            }
            if was_interior != Some(interior) {
                if inside {
                    tracker_leave_interior(g, t);
                }
            } else if room != was_room {
                tracker_set_room(g, t, room);
            }
            g.set_tracker_interior(t, Some(interior));
            g.set_tracker_room(t, room);
            if let Some(e) = e {
                g.set_entity_room(e, room);
                let handle = g.interior_handle(interior);
                g.set_entity_interior_handle(e, Some(handle));
            }
            tracker_enter_room(g, interior, t);
        }
        _ => {
            if inside {
                tracker_leave_interior(g, t);
                if let Some(e) = e {
                    g.entity_went_outside(e);
                }
            }
        }
    }
    tracker_player_outside(g, t);
}

pub fn tracker_watch<G: TrackerRooms>(g: &mut G, t: G::Tracker) -> bool {
    if g.tracker_listed_watched(t) {
        return false;
    }
    if g.is_camera_tracker(t) {
        put_on_watched(g, t);
        return true;
    }
    let Some(e) = g.tracker_entity(t) else { return false };
    let wanted = match g.entity_type(e) {
        EntityType::Ped | EntityType::Vehicle | EntityType::Object => g.entity_made_for_mission(e),
        _ => g.entity_status(e) == EntityStatus::Player,
    };
    if wanted {
        put_on_watched(g, t);
    }
    false
}

fn put_on_watched<G: TrackerRooms>(g: &mut G, t: G::Tracker) {
    let list = g.watched_trackers();
    let item = g.tracker_item(t);
    list_push(g, list, item);
    g.set_tracker_listed_watched(t, true);
}

pub fn tracker_set_watched<G: TrackerRooms>(g: &mut G, t: G::Tracker, state: u8) {
    if g.tracker_watched_state(t) == state {
        return;
    }
    if state == 1 {
        tracker_watch(g, t);
    } else if g.tracker_listed_watched(t) {
        let list = g.watched_trackers();
        let item = g.tracker_item(t);
        list_remove(g, list, item);
        g.set_tracker_listed_watched(t, false);
    }
    g.set_tracker_watched_state(t, state);
    let Some(interior) = g.tracker_interior(t) else { return };
    let Some(e) = g.tracker_entity(t) else { return };
    if !g.entity_flag(e, EntityFlag::Inside) {
        return;
    }
    if state != 0 {
        show_interior(g, interior);
    } else {
        hide_interior(g, interior);
    }
}

pub fn tracker_lost<G: TrackerRooms>(g: &mut G, t: G::Tracker) {
    let Some(e) = g.tracker_entity(t) else { return };
    let kind = g.entity_type(e);
    if kind == EntityType::Object || kind == EntityType::Vehicle {
        g.set_entity_lost_interior_2(e);
        g.entity_lost_interior(e);
    }
    tracker_set_interior(g, t, None, RoomId::SHELL);
    g.set_entity_flag(e, EntityFlag::LostInterior, true);
    tracker_set_watched(g, t, 0);
}
