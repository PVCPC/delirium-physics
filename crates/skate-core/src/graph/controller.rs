use super::{
    activation::{ActivationProgram, ConditionHost},
    selection::{Selection, StateId, Topology, TransitionId},
};

pub type BehaviorId = usize;
pub type HookId = usize;

#[derive(Clone, Debug)]
pub struct Behavior {
    pub owner: StateId,
    pub enabled: bool,
}

#[derive(Clone, Debug)]
pub struct Program {
    pub topology: Topology,
    pub root: StateId,
    pub activation: ActivationProgram,
    pub behaviors: Vec<Behavior>,
    pub state_behaviors: Vec<Vec<BehaviorId>>,
    pub transition_hooks: Vec<Vec<HookId>>,
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub dt: f32,
    pub current: Option<StateId>,
    pub last: Option<StateId>,
    pub state_times: Vec<Option<f32>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveBehavior {
    pub behavior: BehaviorId,
    pub instance: u32,
}

pub trait Host: ConditionHost {
    fn context(&self) -> [u32; 6];
    fn allocate(&mut self, behavior: BehaviorId, frame: &Frame) -> u32;
    fn begin(&mut self, behavior: BehaviorId, context: [u32; 6], frame: &Frame);
    fn update(&mut self, behavior: BehaviorId, context: [u32; 6], frame: &Frame);
    fn end(&mut self, behavior: BehaviorId, context: [u32; 6], frame: &Frame);
    fn hook(&mut self, hook: HookId, frame: &Frame);
    fn release(&mut self, instance: u32);
}

#[derive(Clone, Debug)]
pub struct Controller {
    pub frame: Frame,
    pub active: Vec<ActiveBehavior>,
}

impl Controller {
    pub fn new(state_count: usize) -> Self {
        Self {
            frame: Frame {
                dt: 0.0,
                current: None,
                last: None,
                state_times: vec![None; state_count],
            },
            active: Vec::new(),
        }
    }

    pub fn update(&mut self, program: &Program, dt: f32, host: &mut impl Host) {
        self.frame.dt = dt;
        let mut next = None;
        let transition = Selection {
            graph: &program.topology,
            root: program.root,
            current: self.frame.current,
        }
        .next(&mut next, |node, mask| {
            program
                .activation
                .node_activation(node, mask, &self.frame, host)
        });
        let (exiting, entering) = self.prepare_lists(program, transition, next);

        for time in self.frame.state_times.iter_mut().flatten() {
            *time += dt;
        }
        self.exit(program, &exiting, host);
        if let Some(id) = transition {
            for &hook in &program.transition_hooks[id] {
                host.hook(hook, &self.frame);
            }
        }
        self.enter(program, &entering, host);
        for active in &self.active {
            let mut context = host.context();
            context[2] = active.instance;
            host.update(active.behavior, context, &self.frame);
        }
    }

    pub fn end_all_behaviors(&mut self, host: &mut impl Host) {
        while let Some(&active) = self.active.last() {
            let mut context = host.context();
            context[2] = active.instance;
            host.end(active.behavior, context, &self.frame);
            self.active.pop();
            host.release(active.instance);
        }
    }

    fn prepare_lists(
        &mut self,
        program: &Program,
        transition: Option<TransitionId>,
        next: Option<StateId>,
    ) -> (Vec<StateId>, Vec<StateId>) {
        let old_path = path(&program.topology, self.frame.current);
        let boundary = match transition {
            None => next,
            Some(id) => {
                let target = program.topology.transitions[id].target;
                if old_path.contains(&target) {
                    program.topology.states[target].parent
                } else {
                    Some(target)
                }
            }
        };
        let boundary_path = path(&program.topology, boundary);
        let common = old_path
            .iter()
            .zip(&boundary_path)
            .take_while(|(a, b)| a == b)
            .count();
        let exiting = old_path[common..].to_vec();
        let entering = path(&program.topology, next)[common..].to_vec();
        self.frame.last = self.frame.current;
        self.frame.current = next;
        (exiting, entering)
    }

    fn exit(&mut self, program: &Program, exiting: &[StateId], host: &mut impl Host) {
        for index in (0..self.active.len()).rev() {
            let active = self.active[index];
            if exiting.contains(&program.behaviors[active.behavior].owner) {
                let mut context = host.context();
                context[2] = active.instance;
                host.end(active.behavior, context, &self.frame);
                self.active.remove(index);
                host.release(active.instance);
            }
        }
        for &state in exiting {
            self.frame.state_times[state] = None;
        }
    }

    fn enter(&mut self, program: &Program, entering: &[StateId], host: &mut impl Host) {
        for &state in entering {
            self.frame.state_times[state] = Some(0.0);
        }
        for &state in entering {
            for &behavior in &program.state_behaviors[state] {
                if program.behaviors[behavior].enabled {
                    let instance = host.allocate(behavior, &self.frame);
                    self.active.push(ActiveBehavior { behavior, instance });
                    let mut context = host.context();
                    context[2] = instance;
                    host.begin(behavior, context, &self.frame);
                }
            }
        }
    }
}

fn path(graph: &Topology, mut state: Option<StateId>) -> Vec<StateId> {
    let mut result = Vec::new();
    while let Some(id) = state {
        result.push(id);
        state = graph.states[id].parent;
    }
    result.reverse();
    result
}

