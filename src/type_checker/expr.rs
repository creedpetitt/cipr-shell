use crate::ast::{CiprType, NodeId};
use crate::token::{TokenType, Value};
use crate::type_checker::TypeChecker;

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_expr_stmt(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        if let Some(expr_id) = children[0] {
            self.check(expr_id);
        }
        CiprType::Void
    }

    pub(crate) fn check_literal(&mut self, id: NodeId) -> CiprType {
        match &self.arena[id].value {
            Value::Int(_) => CiprType::Int,
            Value::Float(_) => CiprType::Float,
            Value::Str(_) => CiprType::Str,
            Value::Bool(_) => CiprType::Bool,
            Value::Null => CiprType::Unknown, // Null matches anything for now
        }
    }

    pub(crate) fn check_binary(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        let op = self.arena[id].token.token_type;
        let line = self.arena[id].token.line;

        let left = self.check_child(children[0]);
        let right = self.check_child(children[1]);

        if left != CiprType::Unknown && right != CiprType::Unknown && left != right {
            self.error(
                line,
                &format!(
                    "Type mismatch in binary operation: {:?} and {:?}",
                    left, right
                ),
            );
        }

        match op {
            TokenType::Plus | TokenType::Minus | TokenType::Star | TokenType::Slash => {
                if left == CiprType::Int {
                    CiprType::Int
                } else if left == CiprType::Float {
                    CiprType::Float
                } else {
                    if left != CiprType::Unknown {
                        self.error(
                            line,
                            &format!("Invalid operands for arithmetic: {:?}", left),
                        );
                    }
                    CiprType::Unknown
                }
            }
            TokenType::Greater
            | TokenType::GreaterEqual
            | TokenType::Less
            | TokenType::LessEqual
            | TokenType::EqualEqual
            | TokenType::BangEqual => CiprType::Bool,
            _ => CiprType::Unknown,
        }
    }

    pub(crate) fn check_unary(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        let op = self.arena[id].token.token_type;
        let line = self.arena[id].token.line;

        let right = self.check_child(children[0]);

        match op {
            TokenType::Minus => {
                if right != CiprType::Int && right != CiprType::Float && right != CiprType::Unknown
                {
                    self.error(line, "Operand must be Int or Float.");
                }
                right
            }
            TokenType::Bang => CiprType::Bool,
            _ => CiprType::Unknown,
        }
    }

    pub(crate) fn check_logical(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        self.check_child(children[0]);
        self.check_child(children[1]);
        CiprType::Bool
    }
}
