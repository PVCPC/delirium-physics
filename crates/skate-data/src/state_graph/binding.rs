use super::{
    StateGraph,
    attributes::{Attributes, key_hash},
};
use crate::AssetError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Node {
    State(usize),
    Transition(usize),
    Expression(usize),
    Operation(usize),
}

#[derive(Clone, Debug)]
pub struct State {
    pub element: usize,
    pub name: String,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub behaviors: Vec<usize>,
    pub transitions: Vec<usize>,
    pub expression: Option<usize>,
    pub enabled: u8,
    pub active: u8,
    pub interruptibility: u32,
    pub interrupt_ancestor: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Transition {
    pub element: usize,
    pub owner: usize,
    pub target: Option<usize>,
    pub enabled: u8,
    pub priority: u32,
    pub expression: Option<usize>,
    pub hooks: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct Expression {
    pub element: usize,
    pub enabled: u8,
    pub operator: u32,
    pub children: Vec<Node>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationKind {
    Behavior,
    Condition,
    Hook,
}

#[derive(Clone, Debug)]
pub struct Operation {
    pub element: usize,
    pub parent: Node,
    pub kind: OperationKind,
    pub name: String,
    pub enabled: u8,
    pub condition_mask: Option<u32>,
    pub parameters: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct Binding {
    pub states: Vec<State>,
    pub transitions: Vec<Transition>,
    pub expressions: Vec<Expression>,
    pub operations: Vec<Operation>,
    pub root: usize,
}

#[derive(Clone, Debug)]
pub struct OperationInstances<T> {
    pub operations: Vec<T>,
}

pub trait OperationFactory {
    type Instance;
    type Error: std::fmt::Display;

    fn create(
        &mut self,
        kind: OperationKind,
        parent: Node,
        attributes: &Attributes<'_>,
    ) -> Result<Option<Self::Instance>, Self::Error>;

    fn add_parameter(
        &mut self,
        instance: &mut Self::Instance,
        attributes: &Attributes<'_>,
    ) -> Result<(), Self::Error>;
}

impl Binding {
    pub fn from_graph(source: &StateGraph) -> Result<Self, AssetError> {
        let mut result = Self {
            states: Vec::new(),
            transitions: Vec::new(),
            expressions: Vec::new(),
            operations: Vec::new(),
            root: 0,
        };
        let mut parents = vec![None; source.elements.len()];
        let mut nodes = vec![None; source.elements.len()];
        for (i, element) in source.elements.iter().enumerate() {
            for &child in &element.children {
                if child <= i || child >= parents.len() || parents[child].replace(i).is_some() {
                    return Err(error(source, i, "invalid preorder ownership"));
                }
            }
            let parent = parents[i].and_then(|p| nodes[p]);
            let attrs = Attributes::new(&element.attributes);
            let enabled = if parent.is_some_and(|p| result.enabled(p) == 0) {
                0
            } else {
                attrs.boolean_byte("enabled", 1)
            };
            let name = attrs.text("name").unwrap_or("__unknown__").to_owned();
            let tag = key_hash(&element.tag);
            let node = if tag == key_hash("state") {
                let parent = match parent {
                    None if i == 0 => None,
                    Some(Node::State(id)) => Some(id),
                    _ => return Err(error(source, i, "state requires a state parent")),
                };
                let interruptibility = match attrs.text("interruptable").unwrap_or("true") {
                    "true" => 1,
                    "false" => 0,
                    _ => 2,
                };
                let id = result.states.len();
                result.states.push(State {
                    element: i,
                    name,
                    parent,
                    children: Vec::new(),
                    behaviors: Vec::new(),
                    transitions: Vec::new(),
                    expression: None,
                    enabled,
                    active: attrs.boolean_byte("active", 1),
                    interruptibility,
                    interrupt_ancestor: None,
                });
                if let Some(parent) = parent {
                    result.states[parent].children.push(id);
                }
                Node::State(id)
            } else if tag == key_hash("transition") {
                let Some(Node::State(owner)) = parent else {
                    return Err(error(source, i, "transition requires a state parent"));
                };
                let id = result.transitions.len();
                let priority = match attrs.text("priority") {
                    Some("med") => 1,
                    Some("high") => 2,
                    Some("urgent") => 3,
                    _ => 0,
                };
                result.transitions.push(Transition {
                    element: i,
                    owner,
                    target: None,
                    enabled,
                    priority,
                    expression: None,
                    hooks: Vec::new(),
                });
                result.states[owner].transitions.push(id);
                Node::Transition(id)
            } else if tag == key_hash("expression") {
                let id = result.expressions.len();
                let operator = match attrs.text("op") {
                    Some("and") => 1,
                    Some("or") => 2,
                    Some("not") => 3,
                    _ => 0,
                };
                result.expressions.push(Expression {
                    element: i,
                    enabled,
                    operator,
                    children: Vec::new(),
                });
                match parent {
                    Some(Node::State(owner)) => result.states[owner].expression = Some(id),
                    Some(Node::Transition(owner)) => {
                        result.transitions[owner].expression = Some(id)
                    }
                    Some(Node::Expression(owner)) => result.expressions[owner]
                        .children
                        .push(Node::Expression(id)),
                    _ => {
                        return Err(error(
                            source,
                            i,
                            "expression requires state, transition or expression parent",
                        ));
                    }
                }
                Node::Expression(id)
            } else if tag == key_hash("param") {
                let Some(Node::Operation(owner)) = parent else {
                    return Err(error(source, i, "unbound parameter handler"));
                };
                result.operations[owner].parameters.push(i);
                if !element.children.is_empty() {
                    return Err(error(
                        source,
                        i,
                        "parameter children have no native object parent",
                    ));
                }
                continue;
            } else {
                let kind = if tag == key_hash("behaviour") {
                    OperationKind::Behavior
                } else if tag == key_hash("condition") {
                    OperationKind::Condition
                } else if tag == key_hash("hook") {
                    OperationKind::Hook
                } else {
                    return Err(error(source, i, "unsupported graph element"));
                };
                let Some(parent) = parent else {
                    return Err(error(source, i, "operation requires a parent"));
                };
                let id = result.operations.len();
                match (kind, parent) {
                    (OperationKind::Behavior, Node::State(owner)) => {
                        result.states[owner].behaviors.push(id)
                    }
                    (OperationKind::Condition, Node::Expression(owner)) => {
                        result.expressions[owner].children.push(Node::Operation(id))
                    }
                    (OperationKind::Hook, Node::Transition(owner)) => {
                        result.transitions[owner].hooks.push(id)
                    }
                    _ => return Err(error(source, i, "unsupported operation parent")),
                }
                result.operations.push(Operation {
                    element: i,
                    parent,
                    kind,
                    name,
                    enabled,
                    condition_mask: (kind == OperationKind::Condition)
                        .then(|| parse_condition_mask(attrs.text("mask"))),
                    parameters: Vec::new(),
                });
                Node::Operation(id)
            };
            nodes[i] = Some(node);
        }
        if result.states.is_empty() {
            return Err(AssetError("Graph has no root state".into()));
        }
        result.process_names(source)?;
        Ok(result)
    }

    pub fn instantiate_operations<F: OperationFactory>(
        &self,
        source: &StateGraph,
        factory: &mut F,
    ) -> Result<OperationInstances<F::Instance>, AssetError> {
        let mut instances = Vec::with_capacity(self.operations.len());
        for operation in &self.operations {
            let attributes = Attributes::new(&source.elements[operation.element].attributes);
            let mut instance = factory
                .create(operation.kind, operation.parent, &attributes)
                .map_err(|e| operation_error(source, operation, &e.to_string()))?
                .ok_or_else(|| operation_error(source, operation, "unregistered operation"))?;
            for &element in &operation.parameters {
                let attributes = Attributes::new(&source.elements[element].attributes);
                factory
                    .add_parameter(&mut instance, &attributes)
                    .map_err(|e| operation_error(source, operation, &e.to_string()))?;
            }
            instances.push(instance);
        }
        Ok(OperationInstances {
            operations: instances,
        })
    }

    fn enabled(&self, node: Node) -> u8 {
        match node {
            Node::State(i) => self.states[i].enabled,
            Node::Transition(i) => self.transitions[i].enabled,
            Node::Expression(i) => self.expressions[i].enabled,
            Node::Operation(i) => self.operations[i].enabled,
        }
    }

    fn process_names(&mut self, source: &StateGraph) -> Result<(), AssetError> {
        for id in 0..self.states.len() {
            let state = &self.states[id];
            if state.interruptibility == 2 {
                let attrs = Attributes::new(&source.elements[state.element].attributes);
                let name = attrs.text("interruptable").unwrap();
                let target = self
                    .find_state(id, name, true)
                    .ok_or_else(|| error(source, state.element, "unresolved interrupt ancestor"))?;
                let mut ancestor = Some(id);
                while ancestor.is_some() && ancestor != Some(target) {
                    ancestor = self.states[ancestor.unwrap()].parent;
                }
                if ancestor.is_none() {
                    return Err(error(
                        source,
                        state.element,
                        "interrupt target is not an ancestor",
                    ));
                }
                self.states[id].interrupt_ancestor = Some(target);
            }
        }
        for id in 0..self.transitions.len() {
            let transition = &self.transitions[id];
            let attrs = Attributes::new(&source.elements[transition.element].attributes);
            let target =
                self.find_state(transition.owner, attrs.text("target").unwrap_or(""), true);
            self.transitions[id].target = target;
            if target.is_none_or(|i| self.states[i].enabled == 0) {
                self.transitions[id].enabled = 0;
            }
        }
        Ok(())
    }

    pub fn find_state(&self, start: usize, name: &str, ascend: bool) -> Option<usize> {
        if let Some((first, rest)) = name.split_once('.') {
            let found = self.find_state(start, first, true)?;
            return self.find_state(found, rest, false);
        }
        let key = name_hash(name);
        let mut state = start;
        loop {
            for &child in &self.states[state].children {
                if name_hash(&self.states[child].name) == key {
                    return Some(child);
                }
            }
            if !ascend {
                return None;
            }
            if name_hash(&self.states[state].name) == key {
                return Some(state);
            }
            state = self.states[state].parent?;
        }
    }
}

pub fn name_hash(text: &str) -> u32 {
    text.bytes().fold(0x811c9dc5_u32, |hash, byte| {
        hash.wrapping_mul(0x01000193) ^ u32::from(byte)
    })
}

pub fn parse_condition_mask(value: Option<&str>) -> u32 {
    match value {
        Some("sustain") => 2,
        Some("always") => 3,
        Some("postcond") => 4,
        _ => 1,
    }
}

fn error(source: &StateGraph, element: usize, message: &str) -> AssetError {
    let element = &source.elements[element];
    AssetError(format!(
        "Graph {} at byte {}: {message}",
        element.tag, element.source_offset
    ))
}

fn operation_error(source: &StateGraph, operation: &Operation, message: &str) -> AssetError {
    error(
        source,
        operation.element,
        &format!(
            "{} `{}`: {message}",
            operation_kind(operation.kind),
            operation.name
        ),
    )
}

fn operation_kind(kind: OperationKind) -> &'static str {
    match kind {
        OperationKind::Behavior => "behaviour",
        OperationKind::Condition => "condition",
        OperationKind::Hook => "hook",
    }
}
