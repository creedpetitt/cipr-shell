use crate::ast::{CiprType, NodeId};
use crate::codegen::Codegen;
use inkwell::values::BasicValueEnum;

impl<'a, 'ctx> Codegen<'a, 'ctx> {
    pub(crate) fn get_struct_field_index(
        &self,
        struct_name: &str,
        field_name: &str,
    ) -> Result<u32, String> {
        let (_, fields) = self
            .struct_types
            .get(struct_name)
            .ok_or_else(|| format!("Unknown struct {}", struct_name))?;
        for (i, f) in fields.iter().enumerate() {
            if f == field_name {
                return Ok(i as u32);
            }
        }
        Err(format!(
            "Field '{}' not found in struct '{}'",
            field_name, struct_name
        ))
    }

    pub(crate) fn visit_struct_decl(&mut self, id: NodeId) -> Result<(), String> {
        let name = self.arena[id].token.lexeme.clone();
        let mut field_names = Vec::new();
        let mut field_types = Vec::new();
        for child_opt in &self.arena[id].children {
            if let Some(child_id) = child_opt {
                let field_node = &self.arena[*child_id];
                let field_name = field_node.token.lexeme.clone();
                field_names.push(field_name);
                let field_type = field_node.resolved_type.clone();
                let llvm_type = self.get_llvm_type(&field_type)?;
                field_types.push(llvm_type.into());
            }
        }
        let struct_type = self.context.struct_type(&field_types, false);
        self.struct_types.insert(name, (struct_type, field_names));
        Ok(())
    }

    pub(crate) fn visit_struct_init(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let cipr_type = self.arena[id].resolved_type.clone();
        let struct_name = match cipr_type {
            CiprType::Struct(name) => name,
            _ => return Err("StructInit resolved to non-struct type".to_string()),
        };

        let (struct_type, _) = self
            .struct_types
            .get(&struct_name)
            .ok_or_else(|| format!("Unknown struct '{}'", struct_name))?;
        let mut struct_val = struct_type.const_zero();

        for (i, child_opt) in self.arena[id].children.iter().enumerate() {
            let child_id = child_opt
                .ok_or_else(|| "Struct initializer missing field assignment node".to_string())?;
            let assign_node = &self.arena[child_id];
            let val_id = assign_node.children[0]
                .ok_or_else(|| "Struct field initializer missing value".to_string())?;
            let val = self.evaluate(val_id)?;
            struct_val = self
                .builder
                .build_insert_value(struct_val, val, i as u32, "struct_init")
                .map_err(|e| e.to_string())?
                .into_struct_value();
        }

        Ok(struct_val.into())
    }

    pub(crate) fn visit_get_field(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let target_id =
            self.arena[id].children[0].ok_or_else(|| "GetField missing target".to_string())?;
        let target_type = self.arena[target_id].resolved_type.clone();
        let field_name = self.arena[id].token.lexeme.clone();

        let mut is_ptr = false;
        let struct_name = match target_type {
            CiprType::Struct(name) => name,
            CiprType::Pointer(inner) => {
                is_ptr = true;
                match *inner {
                    CiprType::Struct(name) => name,
                    _ => return Err("Dereferenced pointer is not a struct".to_string()),
                }
            }
            _ => return Err("Cannot get field on non-struct".to_string()),
        };

        let field_idx = self.get_struct_field_index(&struct_name, &field_name)?;

        if is_ptr {
            let ptr_val = self.evaluate(target_id)?.into_pointer_value();
            let field_ptr = self
                .builder
                .build_struct_gep(ptr_val, field_idx, "field_gep")
                .map_err(|e| e.to_string())?;
            Ok(self
                .builder
                .build_load(field_ptr, "field_load")
                .map_err(|e| e.to_string())?)
        } else {
            let struct_val = self.evaluate(target_id)?.into_struct_value();
            Ok(self
                .builder
                .build_extract_value(struct_val, field_idx, "field_extract")
                .map_err(|e| e.to_string())?)
        }
    }

    pub(crate) fn visit_assign_field(
        &mut self,
        id: NodeId,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let target_expr_id =
            self.arena[id].children[0].ok_or_else(|| "AssignField missing target".to_string())?;
        let val_expr_id =
            self.arena[id].children[1].ok_or_else(|| "AssignField missing value".to_string())?;

        let target_type = self.arena[target_expr_id].resolved_type.clone();
        let field_name = self.arena[id].token.lexeme.clone();

        let mut is_ptr = false;
        let struct_name = match target_type {
            CiprType::Struct(name) => name,
            CiprType::Pointer(inner) => {
                is_ptr = true;
                match *inner {
                    CiprType::Struct(name) => name,
                    _ => return Err("Dereferenced pointer is not a struct".to_string()),
                }
            }
            _ => return Err("Cannot assign field on non-struct".to_string()),
        };

        let field_idx = self.get_struct_field_index(&struct_name, &field_name)?;

        let struct_ptr = if is_ptr {
            self.evaluate(target_expr_id)?.into_pointer_value()
        } else {
            self.get_eval_pointer(target_expr_id)?
        };

        let field_ptr = self
            .builder
            .build_struct_gep(struct_ptr, field_idx, "assign_field_gep")
            .map_err(|e| e.to_string())?;

        let val = self.evaluate(val_expr_id)?;
        self.builder
            .build_store(field_ptr, val)
            .map_err(|e| e.to_string())?;

        Ok(val)
    }
}
