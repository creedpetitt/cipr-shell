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
            Value::Null => CiprType::Null,
        }
    }

    pub(crate) fn check_binary(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        let op = self.arena[id].token.token_type;
        let line = self.arena[id].token.line;

        let left = self.check_child(children[0]);
        let right = self.check_child(children[1]);

        match op {
            TokenType::Plus | TokenType::Minus | TokenType::Star | TokenType::Slash => {
                match (&left, &right) {
                    (CiprType::Int, CiprType::Int) => CiprType::Int,
                    (CiprType::Float, CiprType::Float) => CiprType::Float,
                    (CiprType::Unknown, _) | (_, CiprType::Unknown) => CiprType::Unknown,
                    _ => {
                        self.error(
                            line,
                            &format!(
                                "Arithmetic operators require matching Int or Float operands, got {:?} and {:?}",
                                left, right
                            ),
                        );
                        CiprType::Unknown
                    }
                }
            }
            TokenType::Greater
            | TokenType::GreaterEqual
            | TokenType::Less
            | TokenType::LessEqual => match (&left, &right) {
                (CiprType::Int, CiprType::Int) | (CiprType::Float, CiprType::Float) => {
                    CiprType::Bool
                }
                (CiprType::Unknown, _) | (_, CiprType::Unknown) => CiprType::Bool,
                _ => {
                    self.error(
                            line,
                            &format!(
                                "Ordering comparisons require matching Int or Float operands, got {:?} and {:?}",
                                left, right
                            ),
                        );
                    CiprType::Unknown
                }
            },
            TokenType::EqualEqual | TokenType::BangEqual => {
                if left == CiprType::Null && right == CiprType::Null {
                    self.error(line, "Cannot compare two untyped null values.");
                    return CiprType::Unknown;
                }

                if left == CiprType::Null && matches!(right, CiprType::Pointer(_)) {
                    self.coerce_null_child(children[0], &right);
                    return CiprType::Bool;
                }
                if right == CiprType::Null && matches!(left, CiprType::Pointer(_)) {
                    self.coerce_null_child(children[1], &left);
                    return CiprType::Bool;
                }

                match (&left, &right) {
                    (CiprType::Int, CiprType::Int)
                    | (CiprType::Float, CiprType::Float)
                    | (CiprType::Bool, CiprType::Bool)
                    | (CiprType::Pointer(_), CiprType::Pointer(_)) => {
                        if left == right {
                            CiprType::Bool
                        } else {
                            self.error(
                                line,
                                &format!(
                                    "Equality comparisons require matching operand types, got {:?} and {:?}",
                                    left, right
                                ),
                            );
                            CiprType::Unknown
                        }
                    }
                    (CiprType::Unknown, _) | (_, CiprType::Unknown) => CiprType::Bool,
                    _ => {
                        self.error(
                            line,
                            &format!(
                                "Equality comparisons are only supported for Int, Float, Bool, and Pointer operands, got {:?} and {:?}",
                                left, right
                            ),
                        );
                        CiprType::Unknown
                    }
                }
            }
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
                    CiprType::Unknown
                } else {
                    right
                }
            }
            TokenType::Bang => {
                if right != CiprType::Bool && right != CiprType::Unknown {
                    self.error(line, "Operand of '!' must be Bool.");
                    CiprType::Unknown
                } else {
                    CiprType::Bool
                }
            }
            _ => CiprType::Unknown,
        }
    }

    pub(crate) fn check_logical(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        let line = self.arena[id].token.line;
        let left = self.check_child(children[0]);
        let right = self.check_child(children[1]);

        let left_ok = left == CiprType::Bool || left == CiprType::Unknown;
        let right_ok = right == CiprType::Bool || right == CiprType::Unknown;

        if !left_ok || !right_ok {
            self.error(
                line,
                &format!(
                    "Logical operators require Bool operands, got {:?} and {:?}",
                    left, right
                ),
            );
            CiprType::Unknown
        } else {
            CiprType::Bool
        }
    }
}
