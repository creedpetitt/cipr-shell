use crate::ast::{CiprType, NodeId, NodeType};
use crate::type_checker::TypeChecker;

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_addressof(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        let target_id = children[0].expect("AddressOf missing target");
        let target_type = self.check(target_id);

        let target_node_type = self.arena[target_id].node_type;
        if target_node_type != NodeType::VarExpr
            && target_node_type != NodeType::Dereference
            && target_node_type != NodeType::IndexGet
        {
            self.error(
                self.arena[id].token.line,
                "Can only take the address of a variable, dereference, or array index.",
            );
            return CiprType::Unknown;
        }

        CiprType::Pointer(Box::new(target_type))
    }

    pub(crate) fn check_dereference(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        let target_id = children[0].expect("Dereference missing target");
        let target_type = self.check(target_id);

        match target_type {
            CiprType::Pointer(inner) => *inner,
            CiprType::Unknown => CiprType::Unknown,
            _ => {
                self.error(
                    self.arena[id].token.line,
                    "Cannot dereference a non-pointer type.",
                );
                CiprType::Unknown
            }
        }
    }

    pub(crate) fn check_assign_deref(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        let ptr_expr_id = children[0].expect("AssignDeref missing target");
        let val_expr_id = children[1].expect("AssignDeref missing value");

        let ptr_type = self.check(ptr_expr_id);
        let val_type = self.check(val_expr_id);

        let expected_type = match ptr_type {
            CiprType::Pointer(inner) => *inner,
            CiprType::Unknown => CiprType::Unknown,
            _ => {
                self.error(
                    self.arena[id].token.line,
                    "Cannot assign to dereference of a non-pointer type.",
                );
                CiprType::Unknown
            }
        };

        if expected_type != CiprType::Unknown
            && val_type != CiprType::Unknown
            && expected_type != val_type
        {
            self.error(
                self.arena[id].token.line,
                &format!(
                    "Cannot assign {:?} to dereferenced pointer of type {:?}",
                    val_type, expected_type
                ),
            );
        }

        expected_type
    }

    pub(crate) fn check_new(&mut self, id: NodeId) -> CiprType {
        let struct_name = self.arena[id].token.lexeme.clone();

        let struct_fields_opt = self.structs.get(&struct_name).cloned();
        let struct_fields = match struct_fields_opt {
            Some(f) => f,
            None => {
                self.error(
                    self.arena[id].token.line,
                    &format!("Undefined struct '{}' for new", struct_name),
                );
                return CiprType::Unknown;
            }
        };

        let init_nodes = self.arena[id].children.clone();
        if init_nodes.len() != struct_fields.len() {
            self.error(
                self.arena[id].token.line,
                &format!(
                    "'new {}' expects {} arguments but got {}",
                    struct_name,
                    struct_fields.len(),
                    init_nodes.len()
                ),
            );
            return CiprType::Unknown;
        }

        for (i, child_opt) in init_nodes.iter().enumerate() {
            let child_id = child_opt.unwrap();
            let val_type = self.check(child_id);
            let (expected_name, expected_type) = &struct_fields[i];

            if expected_type != &CiprType::Unknown
                && val_type != CiprType::Unknown
                && expected_type != &val_type
            {
                self.error(
                    self.arena[id].token.line,
                    &format!(
                        "'new {}' argument {} ({}) expects type {:?}, got {:?}",
                        struct_name, i, expected_name, expected_type, val_type
                    ),
                );
            }
        }

        CiprType::Pointer(Box::new(CiprType::Struct(struct_name)))
    }

    pub(crate) fn check_delete(&mut self, id: NodeId) -> CiprType {
        let child_id = self.arena[id].children[0].unwrap();
        let child_type = self.check(child_id);

        if let CiprType::Pointer(_) = child_type {
            CiprType::Void
        } else {
            self.error(
                self.arena[id].token.line,
                &format!("Cannot delete non-pointer type {:?}", child_type),
            );
            CiprType::Unknown
        }
    }
}
