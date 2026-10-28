use skate_data::state_graph::binding::{Binding, Node};

pub(super) fn bind(binding: &Binding) -> Vec<bool> {
    let mut gates = vec![false; binding.operations.len()];
    for transition in &binding.transitions {
        let Some(target) = transition.target else { continue; };
        let mut names = Vec::new();
        let mut state = Some(target);
        while let Some(id) = state {
            names.push(binding.states[id].name.as_str());
            state = binding.states[id].parent;
        }
        names.reverse();
        if names != ["Motion", "OnBoard", "OnGround", "RidingIdle", "Riding", "Turning", "Switch"] {
            continue;
        }
        if let Some(expression) = transition.expression {
            mark(binding, expression, &mut gates);
        }
    }
    gates
}
fn mark(binding: &Binding, expression: usize, gates: &mut [bool]) {
    for child in &binding.expressions[expression].children {
        match *child {
            Node::Operation(id) if binding.operations[id].name == "IsRidingFakie" => gates[id] = true,
            Node::Expression(id) => mark(binding, id, gates),
            _ => {}
        }
    }
}
pub(super) fn blocked(enabled: bool, gates: &[bool], operation: usize) -> bool {
    enabled && gates.get(operation).copied().unwrap_or(false)
}

