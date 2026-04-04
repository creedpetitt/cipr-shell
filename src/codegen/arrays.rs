use crate::ast::{CiprType, NodeId};
use crate::codegen::Codegen;
use inkwell::types::BasicType;
use inkwell::values::BasicValueEnum;

impl<'a, 'ctx> Codegen<'a, 'ctx> {
    pub(crate) fn ensure_runtime_oob_function(&self) -> inkwell::values::FunctionValue<'ctx> {
        let fn_type = self.context.void_type().fn_type(
            &[
                self.context.i64_type().into(),
                self.context.i64_type().into(),
            ],
            false,
        );
        self.get_or_add_function("cipr_runtime_oob", fn_type)
    }

    pub(crate) fn get_checked_array_element_ptr(
        &mut self,
        target_id: NodeId,
        index_id: NodeId,
        ptr_name: &str,
    ) -> Result<inkwell::values::PointerValue<'ctx>, String> {
        let target_type = self.arena[target_id].resolved_type.clone();
        let elem_type = match target_type {
            CiprType::Array(inner) => self.get_llvm_type(&inner)?,
            _ => return Err("Only arrays can be indexed".to_string()),
        };

        let array_val = self.evaluate(target_id)?.into_struct_value();
        let index_val = self.evaluate(index_id)?.into_int_value();

        let len_val = self
            .builder
            .build_extract_value(array_val, 0, "arr_len")
            .map_err(|e| e.to_string())?
            .into_int_value();

        let raw_data_ptr = self
            .builder
            .build_extract_value(array_val, 1, "arr_data")
            .map_err(|e| e.to_string())?
            .into_pointer_value();

        let typed_data_ptr = self
            .builder
            .build_pointer_cast(
                raw_data_ptr,
                elem_type.ptr_type(inkwell::AddressSpace::from(0)),
                "arr_data_typed",
            )
            .map_err(|e| e.to_string())?;

        let zero = self.context.i64_type().const_zero();
        let is_negative = self
            .builder
            .build_int_compare(inkwell::IntPredicate::SLT, index_val, zero, "idx_neg")
            .map_err(|e| e.to_string())?;
        let is_too_large = self
            .builder
            .build_int_compare(inkwell::IntPredicate::SGE, index_val, len_val, "idx_oob_hi")
            .map_err(|e| e.to_string())?;
        let is_oob = self
            .builder
            .build_or(is_negative, is_too_large, "idx_oob")
            .map_err(|e| e.to_string())?;

        let parent_fn = self.current_function()?;

        let oob_bb = self.context.append_basic_block(parent_fn, "idx_oob");
        let in_bounds_bb = self.context.append_basic_block(parent_fn, "idx_ok");

        self.builder
            .build_conditional_branch(is_oob, oob_bb, in_bounds_bb)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(oob_bb);
        let oob_fn = self.ensure_runtime_oob_function();
        self.builder
            .build_call(oob_fn, &[index_val.into(), len_val.into()], "")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_unconditional_branch(in_bounds_bb)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(in_bounds_bb);
        unsafe {
            self.builder
                .build_in_bounds_gep(typed_data_ptr, &[index_val], ptr_name)
                .map_err(|e| e.to_string())
        }
    }

    pub(crate) fn visit_array(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let children = self.arena[id].children.clone();
        let len = children.len();

        let elem_type = match &self.arena[id].resolved_type {
            CiprType::Array(t) => self.get_llvm_type(t)?,
            _ => return Err("Expected Array type for array literal".to_string()),
        };

        let size_val = self.context.i32_type().const_int(len as u64, false);
        let ptr = self
            .builder
            .build_array_alloca(elem_type, size_val, "array_alloc")
            .map_err(|e| e.to_string())?;

        for (i, child_opt) in children.iter().enumerate() {
            if let Some(child_id) = child_opt {
                let val = self.evaluate(*child_id)?;
                let idx_val = self.context.i64_type().const_int(i as u64, false);
                let elem_ptr = unsafe {
                    self.builder
                        .build_in_bounds_gep(ptr, &[idx_val], "elem_ptr")
                        .map_err(|e| e.to_string())?
                };
                self.builder
                    .build_store(elem_ptr, val)
                    .map_err(|e| e.to_string())?;
            }
        }

        let array_type = self.get_llvm_type(&self.arena[id].resolved_type)?;
        let mut array_val = array_type.into_struct_type().get_undef();
        let len_val = self.context.i64_type().const_int(len as u64, false);

        array_val = self
            .builder
            .build_insert_value(array_val, len_val, 0, "arr_with_len")
            .map_err(|e| e.to_string())?
            .into_struct_value();

        array_val = self
            .builder
            .build_insert_value(array_val, ptr, 1, "arr_with_ptr")
            .map_err(|e| e.to_string())?
            .into_struct_value();

        Ok(array_val.into())
    }

    pub(crate) fn visit_index_get(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let children = self.arena[id].children.clone();
        let target_id = children[0].expect("Missing array target");
        let index_id = children[1].expect("Missing index");

        let elem_ptr = self.get_checked_array_element_ptr(target_id, index_id, "idx_ptr")?;

        Ok(self
            .builder
            .build_load(elem_ptr, "idx_load")
            .map_err(|e| e.to_string())?)
    }
}
