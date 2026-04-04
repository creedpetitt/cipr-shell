use crate::ast::{CiprType, NodeId};
use crate::codegen::Codegen;
use inkwell::types::BasicType;
use inkwell::values::BasicValueEnum;

impl<'a, 'ctx> Codegen<'a, 'ctx> {
    pub(crate) fn visit_new(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let struct_name = self.arena[id].token.lexeme.clone();
        let (struct_type, _) = self
            .struct_types
            .get(&struct_name)
            .ok_or_else(|| format!("Unknown struct '{}'", struct_name))?;

        let alloc_size_bytes = struct_type
            .size_of()
            .ok_or_else(|| format!("Could not compute size for struct '{}'", struct_name))?;

        // Auto-declare cipr_malloc if not yet in module (i8* cipr_malloc(i64))
        let fn_type = self
            .i8_ptr_type()
            .fn_type(&[self.context.i64_type().into()], false);
        let malloc_fn = self.get_or_add_function("cipr_malloc", fn_type);

        let call_site = self
            .builder
            .build_call(malloc_fn, &[alloc_size_bytes.into()], "malloc_call")
            .map_err(|e| e.to_string())?;

        let raw_ptr = match call_site.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(v) => v.into_pointer_value(),
            _ => return Err("cipr_malloc did not return a pointer".to_string()),
        };

        // Cast raw i8* pointer to Struct pointer type
        let struct_ptr_type = struct_type.ptr_type(inkwell::AddressSpace::from(0));
        let struct_ptr = self
            .builder
            .build_pointer_cast(raw_ptr, struct_ptr_type, "new_ptr_cast")
            .map_err(|e| e.to_string())?;

        // Initialize fields with args
        for (i, child_opt) in self.arena[id].children.iter().enumerate() {
            if let Some(arg_id) = child_opt {
                let val = self.evaluate(*arg_id)?;
                let field_ptr = self
                    .builder
                    .build_struct_gep(struct_ptr, i as u32, "new_gep")
                    .map_err(|e| e.to_string())?;
                self.builder
                    .build_store(field_ptr, val)
                    .map_err(|e| e.to_string())?;
            }
        }

        Ok(struct_ptr.into())
    }

    pub(crate) fn visit_delete(&mut self, id: NodeId) -> Result<(), String> {
        let child_id =
            self.arena[id].children[0].ok_or_else(|| "Delete missing operand".to_string())?;
        let child_type = self.arena[child_id].resolved_type.clone();
        let val = self.evaluate(child_id)?;

        let (ext_name, fn_type, arg) = match child_type {
            CiprType::Str => {
                let struct_type = self.get_llvm_type(&CiprType::Str)?;
                let ftype = self
                    .context
                    .void_type()
                    .fn_type(&[struct_type.into()], false);
                ("cipr_str_free", ftype, val.into())
            }
            CiprType::Pointer(inner) => {
                let (fn_name, arg_val) = match *inner {
                    CiprType::Struct(name) if name == "IntVec" => ("cipr_int_vec_free", val.into()),
                    CiprType::Struct(name) if name == "StrVec" => ("cipr_str_vec_free", val.into()),
                    CiprType::Struct(name) if name == "StrIntMap" => {
                        ("cipr_str_int_map_free", val.into())
                    }
                    CiprType::Struct(name) if name == "StrStrMap" => {
                        ("cipr_str_str_map_free", val.into())
                    }
                    _ => {
                        let raw_ptr = self
                            .builder
                            .build_pointer_cast(
                                val.into_pointer_value(),
                                self.i8_ptr_type(),
                                "delete_ptr_cast",
                            )
                            .map_err(|e| e.to_string())?;
                        ("cipr_free", raw_ptr.into())
                    }
                };
                let ftype = self
                    .context
                    .void_type()
                    .fn_type(&[self.i8_ptr_type().into()], false);
                (fn_name, ftype, arg_val)
            }
            _ => return Err(format!("Cannot delete non-heap type: {:?}", child_type)),
        };

        let free_fn = self.get_or_add_function(ext_name, fn_type);
        self.builder
            .build_call(free_fn, &[arg], "")
            .map_err(|e| e.to_string())?;

        Ok(())
    }
}
