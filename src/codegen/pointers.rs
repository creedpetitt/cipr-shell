use crate::ast::{CiprType, NodeId, NodeType};
use crate::codegen::Codegen;
use inkwell::values::BasicValueEnum;

impl<'a, 'ctx> Codegen<'a, 'ctx> {
    pub(crate) fn get_eval_pointer(
        &mut self,
        id: NodeId,
    ) -> Result<inkwell::values::PointerValue<'ctx>, String> {
        let node_type = self.arena[id].node_type;
        match node_type {
            NodeType::VarExpr => {
                let name = self.arena[id].token.lexeme.clone();
                match self.symbol_table.get(&name) {
                    Some(p) => Ok(p),
                    None => Err(format!("Undefined variable accessing pointer: {}", name)),
                }
            }
            NodeType::Dereference => {
                let inner_id = self.arena[id].children[0].unwrap();
                Ok(self.evaluate(inner_id)?.into_pointer_value())
            }
            NodeType::GetField => {
                let target_id = self.arena[id].children[0].unwrap();
                let field_name = self.arena[id].token.lexeme.clone();
                let target_type = self.arena[target_id].resolved_type.clone();
                let mut is_ptr = false;
                let struct_name = match target_type {
                    CiprType::Struct(name) => name,
                    CiprType::Pointer(inner) => {
                        is_ptr = true;
                        match *inner {
                            CiprType::Struct(n) => n,
                            _ => return Err("Dereferencing non-struct".to_string()),
                        }
                    }
                    _ => return Err("Invalid target for GetField pointer".to_string()),
                };
                let field_idx = self.get_struct_field_index(&struct_name, &field_name)?;

                let target_ptr = if is_ptr {
                    self.evaluate(target_id)?.into_pointer_value()
                } else {
                    self.get_eval_pointer(target_id)?
                };

                let field_ptr = self
                    .builder
                    .build_struct_gep(target_ptr, field_idx, "nested_gep")
                    .map_err(|e| e.to_string())?;
                Ok(field_ptr)
            }
            NodeType::IndexGet => {
                let arr_id = self.arena[id].children[0].unwrap();
                let index_id = self.arena[id].children[1].unwrap();
                self.get_checked_array_element_ptr(arr_id, index_id, "idx_gep")
            }
            _ => Err("Cannot take pointer to ephemeral expression".to_string()),
        }
    }

    pub(crate) fn visit_addressof(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let target_id = self.arena[id].children[0].unwrap();
        Ok(self.get_eval_pointer(target_id)?.into())
    }

    pub(crate) fn visit_dereference(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let children = self.arena[id].children.clone();
        let target_id = children[0].expect("Dereference missing target");
        let ptr_val = self.evaluate(target_id)?.into_pointer_value();
        Ok(self
            .builder
            .build_load(ptr_val, "deref_load")
            .map_err(|e| e.to_string())?)
    }

    pub(crate) fn visit_assign_deref(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let children = self.arena[id].children.clone();
        let ptr_expr_id = children[0].expect("AssignDeref missing target");
        let val_expr_id = children[1].expect("AssignDeref missing value");

        let ptr_val = self.evaluate(ptr_expr_id)?.into_pointer_value();
        let val = self.evaluate(val_expr_id)?;

        self.builder
            .build_store(ptr_val, val)
            .map_err(|e| e.to_string())?;
        Ok(val)
    }
}
