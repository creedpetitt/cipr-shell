use crate::ast::{CiprType, NodeId};
use crate::codegen::Codegen;
use inkwell::types::BasicMetadataTypeEnum;
use inkwell::values::{BasicValueEnum, CallableValue};

impl<'a, 'ctx> Codegen<'a, 'ctx> {
    pub(crate) fn visit_call(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let children = self.arena[id].children.clone();
        let callee_id = children[0].expect("Call missing callee");
        let callee_name = self.arena[callee_id].token.lexeme.clone();

        if self.arena[callee_id].node_type == crate::ast::NodeType::VarExpr
            && callee_name == "print"
        {
            let arg_id = children[1].expect("Print missing argument");
            return self.emit_print_call(arg_id);
        }

        let callee_type = self.arena[callee_id].resolved_type.clone();
        let (param_types, ret_type) = match callee_type {
            CiprType::Callable(params, ret) => (params, *ret),
            other => return Err(format!("Cannot call non-callable type: {:?}", other)),
        };

        if param_types.len() != children.len().saturating_sub(1) {
            return Err(format!(
                "Argument count mismatch: expected {}, got {}",
                param_types.len(),
                children.len().saturating_sub(1)
            ));
        }

        let callable_struct = self.evaluate(callee_id)?.into_struct_value();
        let fn_ptr_raw = self
            .builder
            .build_extract_value(callable_struct, 0, "call_fn_ptr")
            .map_err(|e| e.to_string())?
            .into_pointer_value();
        let env_ptr = self
            .builder
            .build_extract_value(callable_struct, 1, "call_env_ptr")
            .map_err(|e| e.to_string())?
            .into_pointer_value();

        let mut call_param_types: Vec<BasicMetadataTypeEnum<'ctx>> = Vec::new();
        call_param_types.push(self.i8_ptr_type().into());
        for p in &param_types {
            call_param_types.push(self.get_llvm_type(p)?.into());
        }

        let fn_sig = self.build_function_type(&call_param_types, &ret_type)?;
        let typed_fn_ptr = self
            .builder
            .build_pointer_cast(
                fn_ptr_raw,
                fn_sig.ptr_type(inkwell::AddressSpace::from(0)),
                "typed_call_ptr",
            )
            .map_err(|e| e.to_string())?;

        let mut args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = vec![env_ptr.into()];
        for i in 1..children.len() {
            if let Some(arg_id) = children[i] {
                args.push(self.evaluate(arg_id)?.into());
            }
        }

        let callable = CallableValue::try_from(typed_fn_ptr)
            .map_err(|_| "Failed to convert function pointer to callable".to_string())?;
        let call_site = self
            .builder
            .build_call(callable, &args, "fnptr_call")
            .map_err(|e| e.to_string())?;

        match ret_type {
            CiprType::Void => Ok(self.context.i32_type().const_int(0, false).into()),
            _ => match call_site.try_as_basic_value() {
                inkwell::values::ValueKind::Basic(v) => Ok(v),
                _ => Err("Callable unexpectedly returned void".to_string()),
            },
        }
    }

    pub(crate) fn emit_print_call(
        &mut self,
        arg_id: NodeId,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let arg_val = self.evaluate(arg_id)?;
        let arg_type = &self.arena[arg_id].resolved_type;

        let (ext_name, fn_type, args) = match arg_type {
            CiprType::Str => {
                let struct_type = self.get_llvm_type(&CiprType::Str)?;
                let ftype = self
                    .context
                    .void_type()
                    .fn_type(&[struct_type.into()], false);
                ("cipr_print_str", ftype, vec![arg_val.into()])
            }
            CiprType::Int => {
                let ftype = self
                    .context
                    .void_type()
                    .fn_type(&[self.context.i64_type().into()], false);
                ("cipr_print_int", ftype, vec![arg_val.into()])
            }
            CiprType::Float => {
                let ftype = self
                    .context
                    .void_type()
                    .fn_type(&[self.context.f64_type().into()], false);
                ("cipr_print_float", ftype, vec![arg_val.into()])
            }
            CiprType::Bool => {
                let bool_val = arg_val.into_int_value();
                let as_i64 = self
                    .builder
                    .build_int_z_extend(bool_val, self.context.i64_type(), "bool_to_i64")
                    .map_err(|e| e.to_string())?;
                let ftype = self
                    .context
                    .void_type()
                    .fn_type(&[self.context.i64_type().into()], false);
                ("cipr_print_bool", ftype, vec![as_i64.into()])
            }
            _ => return Err(format!("Unsupported type for print: {:?}", arg_type)),
        };

        let print_fn = self.get_or_add_function(ext_name, fn_type);

        self.builder
            .build_call(print_fn, &args, "")
            .map_err(|e| e.to_string())?;
        Ok(self.context.i32_type().const_int(0, false).into())
    }

    pub(crate) fn visit_extern_fn(&mut self, id: NodeId) -> Result<(), String> {
        let name = self.arena[id].token.lexeme.clone();

        let (param_list, ret_type_enum) = match &self.arena[id].resolved_type {
            crate::ast::CiprType::Callable(params, ret) => (params.clone(), *ret.clone()),
            _ => return Err("Expected Callable type for extern fn".to_string()),
        };

        let mut param_types = Vec::new();
        for p in param_list {
            let p_type: inkwell::types::BasicMetadataTypeEnum = self.get_llvm_type(&p)?.into();
            param_types.push(p_type);
        }

        let fn_type = match ret_type_enum {
            crate::ast::CiprType::Void => self.context.void_type().fn_type(&param_types, false),
            t => {
                let basic = self.get_llvm_type(&t)?;
                match basic {
                    inkwell::types::BasicTypeEnum::IntType(i) => i.fn_type(&param_types, false),
                    inkwell::types::BasicTypeEnum::FloatType(f) => f.fn_type(&param_types, false),
                    inkwell::types::BasicTypeEnum::PointerType(p) => p.fn_type(&param_types, false),
                    inkwell::types::BasicTypeEnum::StructType(s) => s.fn_type(&param_types, false),
                    _ => self.context.i32_type().fn_type(&param_types, false),
                }
            }
        };

        self.module
            .add_function(&name, fn_type, Some(inkwell::module::Linkage::External));
        Ok(())
    }

    pub(crate) fn ensure_wrapper_for_function(
        &mut self,
        name: &str,
        direct_fn: inkwell::values::FunctionValue<'ctx>,
        param_list: &[CiprType],
        ret_type: &CiprType,
    ) -> Result<inkwell::values::FunctionValue<'ctx>, String> {
        if let Some(existing) = self.function_wrappers.get(name) {
            return Ok(*existing);
        }

        let wrapper_name = format!("__cipr_cbwrap_{}", name);
        let i8_ptr_type = self.i8_ptr_type();

        let mut wrapper_params: Vec<BasicMetadataTypeEnum<'ctx>> = vec![i8_ptr_type.into()];
        for p in param_list {
            wrapper_params.push(self.get_llvm_type(p)?.into());
        }

        let wrapper_ty = self.build_function_type(&wrapper_params, ret_type)?;
        let wrapper_fn = self.module.add_function(&wrapper_name, wrapper_ty, None);

        let prev_bb = self.builder.get_insert_block();
        let entry = self.context.append_basic_block(wrapper_fn, "entry");
        self.builder.position_at_end(entry);

        let mut direct_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = Vec::new();
        for (i, p) in param_list.iter().enumerate() {
            let arg = wrapper_fn
                .get_nth_param((i + 1) as u32)
                .ok_or_else(|| "Missing wrapper argument".to_string())?;
            let expected_ty = self.get_llvm_type(p)?;
            if arg.get_type() != expected_ty {
                return Err(format!(
                    "Wrapper arg type mismatch for '{}': expected {:?}, got {:?}",
                    name,
                    expected_ty,
                    arg.get_type()
                ));
            }
            direct_args.push(arg.into());
        }

        let call_site = self
            .builder
            .build_call(direct_fn, &direct_args, &format!("{}_direct_call", name))
            .map_err(|e| e.to_string())?;

        match ret_type {
            CiprType::Void => {
                self.builder.build_return(None).map_err(|e| e.to_string())?;
            }
            _ => match call_site.try_as_basic_value() {
                inkwell::values::ValueKind::Basic(v) => {
                    self.builder
                        .build_return(Some(&v))
                        .map_err(|e| e.to_string())?;
                }
                _ => return Err(format!("Function '{}' wrapper expected return value", name)),
            },
        }

        if let Some(bb) = prev_bb {
            self.builder.position_at_end(bb);
        }

        self.function_wrappers.insert(name.to_string(), wrapper_fn);
        Ok(wrapper_fn)
    }

    pub(crate) fn visit_function(&mut self, id: NodeId) -> Result<(), String> {
        let name = self.arena[id].token.lexeme.clone();
        let children = self.arena[id].children.clone();
        let param_count = children.len() - 1;

        // Determine return type
        let ret_type_enum = match &self.arena[id].resolved_type {
            crate::ast::CiprType::Callable(_, ret) => *ret.clone(),
            _ => crate::ast::CiprType::Void,
        };

        // Determine parameter types
        let mut param_types = Vec::new();
        for i in 0..param_count {
            if let Some(param_id) = children[i] {
                let p_type: BasicMetadataTypeEnum = self
                    .get_llvm_type(&self.arena[param_id].resolved_type)?
                    .into();
                param_types.push(p_type);
            }
        }

        let fn_type = match ret_type_enum {
            CiprType::Void => self.context.void_type().fn_type(&param_types, false),
            _ => {
                let ret_type = self.get_llvm_type(&ret_type_enum)?;
                match ret_type {
                    inkwell::types::BasicTypeEnum::IntType(t) => t.fn_type(&param_types, false),
                    inkwell::types::BasicTypeEnum::FloatType(t) => t.fn_type(&param_types, false),
                    inkwell::types::BasicTypeEnum::StructType(t) => t.fn_type(&param_types, false),
                    inkwell::types::BasicTypeEnum::PointerType(t) => t.fn_type(&param_types, false),
                    _ => {
                        return Err(format!(
                            "Unsupported function return type: {:?}",
                            ret_type_enum
                        ))
                    }
                }
            }
        };
        let function = self.module.add_function(&name, fn_type, None);

        // Save current insertion point
        let original_bb = self.builder.get_insert_block();

        // Create entry block
        let entry_bb = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry_bb);

        // New scope for function body
        self.symbol_table.enter_scope();

        // Allocate and store parameters
        for (i, arg) in function.get_param_iter().enumerate() {
            if let Some(param_id) = children[i] {
                let p_name = self.arena[param_id].token.lexeme.clone();
                let p_type = self.get_llvm_type(&self.arena[param_id].resolved_type)?;
                let alloca = self
                    .builder
                    .build_alloca(p_type, &p_name)
                    .map_err(|e| e.to_string())?;
                self.builder
                    .build_store(alloca, arg)
                    .map_err(|e| e.to_string())?;
                self.symbol_table.define(&p_name, alloca);
            }
        }

        // Compile body
        if let Some(body_id) = children[children.len() - 1] {
            self.execute(body_id)?;
        }

        // Add implicit return if missing
        if self
            .builder
            .get_insert_block()
            .ok_or("Lost track of block")?
            .get_terminator()
            .is_none()
        {
            if matches!(ret_type_enum, CiprType::Void) {
                self.builder.build_return(None).map_err(|e| e.to_string())?;
            } else {
                return Err(format!(
                    "Function '{}' may exit without returning a value of type {:?}",
                    name, ret_type_enum
                ));
            }
        }

        // Restore scope and insertion point
        self.symbol_table.exit_scope();
        if let Some(bb) = original_bb {
            self.builder.position_at_end(bb);
        }

        Ok(())
    }
}
