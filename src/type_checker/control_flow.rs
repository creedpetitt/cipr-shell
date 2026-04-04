use crate::ast::{CiprType, NodeId};
use crate::type_checker::TypeChecker;

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_block(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        self.predeclare_structs(&children);
        for child in children.iter().flatten() {
            self.check(*child);
        }
        CiprType::Void
    }

    pub(crate) fn check_block_stmt(&mut self, id: NodeId) -> CiprType {
        self.env.enter_scope();

        let children = self.arena[id].children.clone();
        self.predeclare_structs(&children);
        for child in children.iter().flatten() {
            self.check(*child);
        }

        self.env.exit_scope();
        CiprType::Void
    }

    pub(crate) fn check_if(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        let line = self.arena[id].token.line;

        let cond_type = self.check_child(children[0]);
        if cond_type != CiprType::Bool && cond_type != CiprType::Unknown {
            self.error(line, "Condition must be a boolean.");
        }

        self.check_child(children[1]);

        if let Some(else_id) = children.get(2).copied().flatten() {
            self.check(else_id);
        }
        CiprType::Void
    }

    pub(crate) fn check_while(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        let line = self.arena[id].token.line;

        let cond_type = self.check_child(children[0]);
        if cond_type != CiprType::Bool && cond_type != CiprType::Unknown {
            self.error(line, "Condition must be a boolean.");
        }

        self.check_child(children[1]);
        CiprType::Void
    }

    pub(crate) fn check_return(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        let line = self.arena[id].token.line;

        let val_type = self.check_child(children[0]);

        if let Some(expected) = self.current_return_type.clone() {
            if expected == CiprType::Void && children[0].is_some() {
                self.error(line, "Void functions cannot return a value.");
            } else if val_type == CiprType::Null && !self.coerce_null_child(children[0], &expected)
            {
                self.error(line, "Cannot return null from a non-pointer function.");
            } else if !self.types_match(&expected, &val_type)
                && expected != CiprType::Void
                && val_type != CiprType::Unknown
            {
                self.error(
                    line,
                    &format!("Expected return type {:?} but got {:?}", expected, val_type),
                );
            }
        } else {
            self.error(line, "Cannot return from top-level code.");
        }

        CiprType::Void
    }
}
