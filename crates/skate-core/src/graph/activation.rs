use super::{controller::Frame, expression::evaluate_operator, selection::NodeId};

pub type ConditionId = usize;
pub type ExpressionId = usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Child {
    Expression(ExpressionId),
    Condition(ConditionId),
}

#[derive(Clone, Debug)]
pub struct Expression {
    pub operator: u32,
    pub children: Vec<Child>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Condition {
    pub enabled: bool,
    pub mask: u32,
}

#[derive(Clone, Debug)]
pub struct ActivationProgram {
    pub state_expressions: Vec<Option<ExpressionId>>,
    pub transition_expressions: Vec<Option<ExpressionId>>,
    pub expressions: Vec<Expression>,
    pub conditions: Vec<Condition>,
}

pub trait ConditionHost {
    fn condition_activation(&mut self, condition: ConditionId, frame: &Frame) -> u32;
}

impl ActivationProgram {
    pub fn node_activation(
        &self,
        node: NodeId,
        mask: u32,
        frame: &Frame,
        host: &mut impl ConditionHost,
    ) -> bool {
        let expression = match node {
            NodeId::State(id) => self.state_expressions[id],
            NodeId::Transition(id) => self.transition_expressions[id],
        };
        let Some(expression) = expression else {
            return true;
        };
        let mut excluded = 1;
        let result = self.expression_activation(expression, mask, frame, &mut excluded, host);
        excluded != 0 || result as u8 != 0
    }

    fn expression_activation(
        &self,
        id: ExpressionId,
        mask: u32,
        frame: &Frame,
        excluded: &mut u8,
        host: &mut impl ConditionHost,
    ) -> u32 {
        let expression = &self.expressions[id];
        evaluate_operator(
            expression.operator,
            expression.children.len(),
            excluded,
            |index, child_excluded| match expression.children[index] {
                Child::Expression(child) => {
                    self.expression_activation(child, mask, frame, child_excluded, host)
                }
                Child::Condition(child) => {
                    self.condition_activation(child, mask, frame, child_excluded, host)
                }
            },
        )
    }

    fn condition_activation(
        &self,
        id: ConditionId,
        mask: u32,
        frame: &Frame,
        excluded: &mut u8,
        host: &mut impl ConditionHost,
    ) -> u32 {
        let condition = self.conditions[id];
        if !condition.enabled || condition.mask & mask == 0 {
            *excluded = 1;
            1
        } else {
            *excluded = 0;
            host.condition_activation(id, frame)
        }
    }
}

