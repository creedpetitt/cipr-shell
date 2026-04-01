use crate::ast::{CiprType, NodeId};
use crate::type_checker::TypeChecker;

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_var_decl(&mut self, id: NodeId) -> CiprType {
        let name = self.arena[id].token.lexeme.clone();
        let line = self.arena[id].token.line;
        let annotation = self.arena[id].type_annotation.clone();
        let declared_type = Self::parse_type_annotation(&annotation);

        let children = self.arena[id].children.clone();
        let init_type = self.check_child(children[0]);

        let final_type = if declared_type != CiprType::Unknown {
            if init_type != CiprType::Unknown && init_type != declared_type {
                self.error(
                    line,
                    &format!(
                        "Cannot assign {:?} to variable of type {:?}",
                        init_type, declared_type
                    ),
                );
            }
            declared_type
        } else if init_type != CiprType::Unknown {
            init_type
        } else {
            self.error(
                line,
                "Variables must have a type annotation or an initializer.",
            );
            CiprType::Unknown
        };

        self.env.define(&name, final_type.clone());
        final_type
    }

    pub(crate) fn check_assign(&mut self, id: NodeId) -> CiprType {
        let name = self.arena[id].token.lexeme.clone();
        let line = self.arena[id].token.line;
        let children = self.arena[id].children.clone();

        let val_type = self.check_child(children[0]);

        let var_type_opt = self.env.get(&name);
        if let Some(var_type) = var_type_opt {
            if var_type != CiprType::Unknown
                && val_type != CiprType::Unknown
                && var_type != val_type
            {
                self.error(
                    line,
                    &format!(
                        "Cannot assign {:?} to variable of type {:?}",
                        val_type, var_type
                    ),
                );
            }
            var_type
        } else {
            self.error(line, &format!("Undefined variable '{}'.", name));
            CiprType::Unknown
        }
    }

    pub(crate) fn check_var_expr(&mut self, id: NodeId) -> CiprType {
        let name = self.arena[id].token.lexeme.clone();
        let line = self.arena[id].token.line;
        let t_opt = self.env.get(&name);
        if let Some(t) = t_opt {
            t
        } else {
            self.error(line, &format!("Undefined variable '{}'.", name));
            CiprType::Unknown
        }
    }
}
