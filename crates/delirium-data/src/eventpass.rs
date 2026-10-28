use super::*;
use crate::evqueue::{self, Ev, Queue, Records, Reply, Tree};
use std::cell::RefCell;

impl<P: Pedlike> evqueue::Record for Event<P> {
    fn kind(&self) -> u32 {
        self.kind
    }
    fn stacks(&self) -> bool {
        Event::stacks(self)
    }
}

struct Pass<'a, P> {
    ped: &'a mut P,
    tasks: &'a mut Manager<P>,
    handler: &'a dyn Handler<P>,
    script: &'a mut ScriptTask,
    built: Response<P>,
    slot2: bool,
    roots: [u32; 5],
}

type Item<P> = RefCell<Event<P>>;

impl<P: Pedlike> evqueue::Asks<Item<P>> for Pass<'_, P> {
    fn ready(&mut self, e: &mut Ev<Item<P>>) -> bool {
        let d = e.data.get_mut();
        d.age = e.age as u32;
        let r = d.ready(&*self.ped, &*self.tasks);
        e.age = d.age as i32;
        r
    }
    fn valid(&mut self, e: &Ev<Item<P>>) -> bool {
        let mut d = e.data.borrow_mut();
        d.age = e.age as u32;
        d.valid(&*self.ped)
    }
    fn concerns(&mut self, e: &Ev<Item<P>>) -> bool {
        e.data.borrow().concerns(&*self.ped, &*self.tasks)
    }
}

impl<P: Pedlike> evqueue::Handler<Item<P>, Event<P>> for Pass<'_, P> {
    fn overrides(&mut self, e: &Ev<Item<P>>, r: &Event<P>) -> bool {
        e.data.borrow().overrides(r)
    }
    fn temporary(&mut self, e: &Ev<Item<P>>) -> bool {
        e.data.borrow().temporary()
    }
    fn respond(&mut self, e: &Ev<Item<P>>, slot2: &mut bool) -> Reply {
        let d = e.data.borrow();
        self.built = match &d.script {
            Some(s) => Response { main: s.task.clone(), secondary: None },
            None => self.handler.respond(self.ped, &*self.tasks, &d),
        };
        self.slot2 = *slot2;
        Reply { main: self.built.main.is_some(), secondary: self.built.secondary.is_some() }
    }
    fn other(&mut self) -> bool {
        false
    }
    fn other_tree(&mut self) -> u32 {
        0
    }
    fn ask(&mut self, task: u32, urgency: u32, e: &mut Ev<Item<P>>) -> bool {
        let p = self.roots.iter().position(|&r| r == task).expect("the pass asks a primary root");
        let d = e.data.get_mut();
        let ev = NewTask {
            secondary: self.built.secondary.is_some(),
            drop_main: Default::default(),
            kind: self.built.main.as_ref().map(|t| t.kind()),
            priority: d.priority().max(0) as usize,
            event: d.kind,
            flag: d.switch.as_ref().is_some_and(|s| s.flag),
            switched: d.switch.as_ref().map(|s| s.task.kind()),
        };
        let urgency = [ABORT_LEAVE, ABORT_URGENT, ABORT_IMMEDIATE][urgency as usize];
        let ok = self.tasks.ask(self.ped, p, urgency, &ev);
        if ev.drop_main.get() {
            self.built.main = None;
        }
        ok
    }
    fn ragdoll(&mut self, e: &Ev<Item<P>>) -> bool {
        let nm_leaf = self.tasks.leaf().is_some_and(|l| (task::NM_RELAX()..=task::NM_FALL_DOWN()).contains(&l.kind()) || l.kind() == task::DEAD());
        self.ped.on_ragdoll() && e.data.borrow().takes_ragdoll() && !nm_leaf
    }
    fn other_stop(&mut self) {}
    fn note(&mut self, recs: &mut Records<Event<P>>, e: &Ev<Item<P>>, slot2: bool) {
        let d = e.data.borrow();
        if self.built.main.is_none() || d.kind == kind::SCRIPT_COMMAND {
            return;
        }
        if d.temporary() && !slot2 {
            if let Some(n) = recs.nontemp.take() {
                recs.pending = Some(n);
            }
            recs.temp = Some(d.record());
        } else {
            (recs.nontemp, recs.pending, recs.temp) = (Some(d.record()), None, None);
        }
    }
    fn note_other(&mut self, _: &mut Records<Event<P>>, _: &Ev<Item<P>>) {}
    fn give(&mut self, e: &Ev<Item<P>>) {
        let d = e.data.borrow();
        let r = std::mem::take(&mut self.built);
        match &d.script {
            Some(s) => {
                self.tasks.clear_responses(self.ped, s.priority);
                if d.kind == kind::SCRIPT_COMMAND {
                    self.script.accepted(d.tag);
                }
                self.tasks.give(self.ped, r.main, s.priority);
                if d.kind == kind::SCRIPT_COMMAND {
                    self.tasks.tag_root(s.priority, d.tag);
                }
            }
            None => give(r, d.temporary() && !self.slot2, self.ped, self.tasks),
        }
    }
    fn clear(&mut self) {
        self.built = Response::default();
    }
}

pub(super) fn handle<P: Pedlike>(evs: &mut Events<P>, ped: &mut P, tasks: &mut Manager<P>, handler: &dyn Handler<P>) {
    let tree = Tree { slots: std::array::from_fn(|p| tasks.root_id(p)), secondary: tasks.secondary_id(0) };
    let ev = |e: Event<P>| Ev { kind: e.kind, priority: e.priority(), lifetime: 0, age: e.age as i32, keep: e.keep, data: RefCell::new(e) };
    let mut q = Queue { evs: evs.queue.drain(..).map(ev).collect() };
    let mut pass = Pass { ped, tasks, handler, script: &mut evs.script, built: Response::default(), slot2: false, roots: tree.slots };
    q.handle(&mut evs.recs, &tree, &mut evs.slot2, &mut pass);
    evs.queue = q.evs.into_iter().map(|e| {
        let mut d = e.data.into_inner();
        d.age = e.age as u32;
        d
    }).collect();
}
