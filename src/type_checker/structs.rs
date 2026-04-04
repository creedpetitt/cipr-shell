use crate::ast::{CiprType, NodeId};
use crate::type_checker::TypeChecker;

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_struct_decl(&mut self, id: NodeId) -> CiprType {
        let name = self.arena[id].token.lexeme.clone();
        if !self.structs.contains_key(&name) {
            let _ = self.register_struct_decl(id);
        }

        let children = self.arena[id].children.clone();
        for child_opt in &children {
            if let Some(child_id) = child_opt {
                let field_type = {
                    let field_node = &self.arena[*child_id];
                    Self::parse_type_annotation(&field_node.type_annotation)
                };
                self.validate_value_type(
                    &field_type,
                    self.arena[*child_id].token.line,
                    "Struct field type",
                );
                self.arena[*child_id].resolved_type = field_type.clone();
            }
        }
        CiprType::Struct(name)
    }

    pub(crate) fn check_struct_init(&mut self, id: NodeId) -> CiprType {
        let struct_name = self.arena[id].token.lexeme.clone();

        let struct_fields_opt = self.structs.get(&struct_name).cloned();
        let struct_fields = match struct_fields_opt {
            Some(f) => f,
            None => {
                self.error(
                    self.arena[id].token.line,
                    &format!("Undefined struct '{}'", struct_name),
                );
                return CiprType::Unknown;
            }
        };

        let init_nodes = self.arena[id].children.clone();
        if init_nodes.len() != struct_fields.len() {
            self.error(
                self.arena[id].token.line,
                &format!(
                    "Struct '{}' expects {} fields but got {}",
                    struct_name,
                    struct_fields.len(),
                    init_nodes.len()
                ),
            );
            return CiprType::Unknown;
        }

        for (i, child_opt) in init_nodes.iter().enumerate() {
            let child_id = child_opt.unwrap();
            let (field_name, val_id, line) = {
                let assign_node = &self.arena[child_id];
                (
                    assign_node.token.lexeme.clone(),
                    assign_node.children[0].unwrap(),
                    assign_node.token.line,
                )
            };

            let (expected_name, expected_type) = &struct_fields[i];
            if &field_name != expected_name {
                self.error(line, &format!("Provided struct field '{}' does not match expected field '{}' at position {}", field_name, expected_name, i));
            }

            let val_type = self.check(val_id);
            if val_type == CiprType::Null && !self.coerce_null_child(Some(val_id), expected_type) {
                self.error(
                    line,
                    "Null can only initialize pointer-typed struct fields.",
                );
            } else if expected_type != &CiprType::Unknown
                && val_type != CiprType::Unknown
                && !self.types_match(expected_type, &val_type)
            {
                self.error(
                    line,
                    &format!(
                        "Field '{}' expects type {:?}, got {:?}",
                        field_name, expected_type, val_type
                    ),
                );
            }
        }

        CiprType::Struct(struct_name)
    }

    pub(crate) fn check_get_field(&mut self, id: NodeId) -> CiprType {
        let target_id = self.arena[id].children[0].unwrap();
        let mut target_type = self.check(target_id);

        // Auto-deref support (p@.field vs p.field)
        if let CiprType::Pointer(inner) = &target_type {
            target_type = *inner.clone();
        }

        let field_name = self.arena[id].token.lexeme.clone();

        match target_type {
            CiprType::Struct(struct_name) => {
                let fields = match self.structs.get(&struct_name) {
                    Some(f) => f,
                    None => return CiprType::Unknown,
                };
                for (name, ty) in fields {
                    if name == &field_name {
                        return ty.clone();
                    }
                }
                self.error(
                    self.arena[id].token.line,
                    &format!("Struct '{}' has no field '{}'", struct_name, field_name),
                );
                CiprType::Unknown
            }
            CiprType::Unknown => CiprType::Unknown,
            _ => {
                self.error(
                    self.arena[id].token.line,
                    "Cannot access field on non-struct type",
                );
                CiprType::Unknown
            }
        }
    }

    pub(crate) fn check_assign_field(&mut self, id: NodeId) -> CiprType {
        let target_id = self.arena[id].children[0].unwrap();
        let val_id = self.arena[id].children[1].unwrap();

        let mut target_type = self.check(target_id);
        let val_type = self.check(val_id);

        if let CiprType::Pointer(inner) = &target_type {
            target_type = *inner.clone();
        }

        let field_name = self.arena[id].token.lexeme.clone();

        let expected_type = match target_type {
            CiprType::Struct(struct_name) => {
                let fields = match self.structs.get(&struct_name) {
                    Some(f) => f,
                    None => return CiprType::Unknown,
                };
                let mut found_type = CiprType::Unknown;
                for (name, ty) in fields {
                    if name == &field_name {
                        found_type = ty.clone();
                        break;
                    }
                }
                if found_type == CiprType::Unknown {
                    self.error(
                        self.arena[id].token.line,
                        &format!("Struct '{}' has no field '{}'", struct_name, field_name),
                    );
                }
                found_type
            }
            CiprType::Unknown => CiprType::Unknown,
            _ => {
                self.error(
                    self.arena[id].token.line,
                    "Cannot assign field on non-struct type",
                );
                CiprType::Unknown
            }
        };

        if val_type == CiprType::Null && !self.coerce_null_child(Some(val_id), &expected_type) {
            self.error(
                self.arena[id].token.line,
                "Null can only be assigned to pointer-typed struct fields.",
            );
        } else if expected_type != CiprType::Unknown
            && val_type != CiprType::Unknown
            && !self.types_match(&expected_type, &val_type)
        {
            self.error(
                self.arena[id].token.line,
                &format!(
                    "Field '{}' expects type {:?}, got {:?}",
                    field_name, expected_type, val_type
                ),
            );
        }

        expected_type
    }
}
