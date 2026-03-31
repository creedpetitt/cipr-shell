use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::{BasicMetadataTypeEnum, BasicType, FunctionType};
use inkwell::values::{BasicValueEnum, CallableValue};

use crate::ast::{CiprType, NodeArena, NodeId, NodeType};
use crate::symbol_table::SymbolTable;
use crate::token::{TokenType, Value};

pub struct Codegen<'a, 'ctx> {
    pub context: &'ctx Context,
    pub builder: &'a Builder<'ctx>,
    pub module: &'a Module<'ctx>,
    pub arena: &'a NodeArena,
    pub symbol_table: SymbolTable<'ctx>,
    pub struct_types:
        std::collections::HashMap<String, (inkwell::types::StructType<'ctx>, Vec<String>)>,
    pub function_wrappers: std::collections::HashMap<String, inkwell::values::FunctionValue<'ctx>>,
}

impl<'a, 'ctx> Codegen<'a, 'ctx> {
    pub fn new(
        context: &'ctx Context,
        builder: &'a Builder<'ctx>,
        module: &'a Module<'ctx>,
        arena: &'a NodeArena,
    ) -> Self {
        Self {
            context,
            builder,
            module,
            arena,
            symbol_table: SymbolTable::new(),
            struct_types: std::collections::HashMap::new(),
            function_wrappers: std::collections::HashMap::new(),
        }
    }

    fn callable_llvm_type(&self) -> inkwell::types::StructType<'ctx> {
        let i8_ptr_type = self
            .context
            .i8_type()
            .ptr_type(inkwell::AddressSpace::from(0));
        self.context
            .struct_type(&[i8_ptr_type.into(), i8_ptr_type.into()], false)
    }

    fn build_function_type(
        &self,
        param_types: &[BasicMetadataTypeEnum<'ctx>],
        ret_type: &CiprType,
    ) -> Result<FunctionType<'ctx>, String> {
        match ret_type {
            CiprType::Void => Ok(self.context.void_type().fn_type(param_types, false)),
            t => {
                let basic = self.get_llvm_type(t)?;
                match basic {
                    inkwell::types::BasicTypeEnum::IntType(i) => Ok(i.fn_type(param_types, false)),
                    inkwell::types::BasicTypeEnum::FloatType(f) => {
                        Ok(f.fn_type(param_types, false))
                    }
                    inkwell::types::BasicTypeEnum::PointerType(p) => {
                        Ok(p.fn_type(param_types, false))
                    }
                    inkwell::types::BasicTypeEnum::StructType(s) => {
                        Ok(s.fn_type(param_types, false))
                    }
                    _ => Err(format!("Unsupported function return type: {:?}", ret_type)),
                }
            }
        }
    }

    fn ensure_wrapper_for_function(
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
        let i8_ptr_type = self
            .context
            .i8_type()
            .ptr_type(inkwell::AddressSpace::from(0));

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

    pub fn compile(&mut self, root_id: NodeId) -> Result<(), String> {
        // Create the top-level `main` function (returns i32)
        let i32_type = self.context.i32_type();
        let fn_type = i32_type.fn_type(&[], false);
        let main_fn = self.module.add_function("main", fn_type, None);
        let basic_block = self.context.append_basic_block(main_fn, "entry");
        self.builder.position_at_end(basic_block);

        let node_type = self.arena[root_id].node_type;
        if node_type == NodeType::StmtList {
            let children = self.arena[root_id].children.clone();
            for child_id in children.iter().flatten() {
                self.execute(*child_id)?;
            }
        }

        // Always return 0 from main if we reach the end
        let zero = self.context.i32_type().const_int(0, false);
        self.builder
            .build_return(Some(&zero))
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn execute(&mut self, id: NodeId) -> Result<(), String> {
        let node_type = self.arena[id].node_type;

        match node_type {
            NodeType::StmtList | NodeType::StmtBlock => {
                self.symbol_table.enter_scope();

                let children = self.arena[id].children.clone();
                for c in children.iter().flatten() {
                    self.execute(*c)?;
                }

                self.symbol_table.exit_scope();
                Ok(())
            }
            NodeType::StmtInclude => {
                let children = self.arena[id].children.clone();
                for c in children.iter().flatten() {
                    self.execute(*c)?;
                }
                Ok(())
            }
            NodeType::StmtExternFn => self.visit_extern_fn(id),
            NodeType::StmtVarDecl => self.visit_var_decl(id),
            NodeType::StmtExpr => {
                let children = self.arena[id].children.clone();
                if let Some(expr_id) = children[0] {
                    self.evaluate(expr_id)?;
                }
                Ok(())
            }
            NodeType::StmtDelete => self.visit_delete(id),
            NodeType::StmtIf => self.visit_if(id),
            NodeType::StmtWhile => self.visit_while(id),
            NodeType::StmtFunction => self.visit_function(id),
            NodeType::StmtReturn => self.visit_return(id),
            NodeType::StmtStructDecl => {
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
            _ => Ok(()), // Skip others for now
        }
    }

    fn visit_if(&mut self, id: NodeId) -> Result<(), String> {
        let children = self.arena[id].children.clone();

        let cond_id = children[0].expect("If missing condition");
        let cond_val = self.evaluate(cond_id)?.into_int_value();

        let parent_fn = self
            .builder
            .get_insert_block()
            .ok_or("Not currently in a basic block")?
            .get_parent()
            .ok_or("Basic block has no parent function")?;

        let then_bb = self.context.append_basic_block(parent_fn, "then");
        let else_bb = self.context.append_basic_block(parent_fn, "else");
        let merge_bb = self.context.append_basic_block(parent_fn, "ifcont");

        let has_else = children.get(2).and_then(|x| *x).is_some();

        if has_else {
            self.builder
                .build_conditional_branch(cond_val, then_bb, else_bb)
                .map_err(|e| e.to_string())?;
        } else {
            self.builder
                .build_conditional_branch(cond_val, then_bb, merge_bb)
                .map_err(|e| e.to_string())?;
        }

        // Then block
        self.builder.position_at_end(then_bb);
        if let Some(then_id) = children[1] {
            self.execute(then_id)?;
        }
        // Only branch to merge if this block doesn't already have a terminator (e.g. a return)
        if self
            .builder
            .get_insert_block()
            .and_then(|bb| bb.get_terminator())
            .is_none()
        {
            self.builder
                .build_unconditional_branch(merge_bb)
                .map_err(|e| e.to_string())?;
        }

        // Else block
        self.builder.position_at_end(else_bb);
        if has_else {
            if let Some(else_id) = children[2] {
                self.execute(else_id)?;
            }
        }
        // Same terminator check for else block
        if self
            .builder
            .get_insert_block()
            .and_then(|bb| bb.get_terminator())
            .is_none()
        {
            self.builder
                .build_unconditional_branch(merge_bb)
                .map_err(|e| e.to_string())?;
        }

        // Continue after if/else
        self.builder.position_at_end(merge_bb);
        Ok(())
    }

    fn visit_while(&mut self, id: NodeId) -> Result<(), String> {
        let children = self.arena[id].children.clone();

        let parent_fn = self
            .builder
            .get_insert_block()
            .ok_or("Not currently in a basic block")?
            .get_parent()
            .ok_or("Basic block has no parent function")?;

        let cond_bb = self.context.append_basic_block(parent_fn, "whilecond");
        let loop_bb = self.context.append_basic_block(parent_fn, "whileloop");
        let after_bb = self.context.append_basic_block(parent_fn, "whilecont");

        // Jump to condition evaluation
        self.builder
            .build_unconditional_branch(cond_bb)
            .map_err(|e| e.to_string())?;

        // Evaluate condition
        self.builder.position_at_end(cond_bb);
        let cond_id = children[0].expect("While missing condition");
        let cond_val = self.evaluate(cond_id)?.into_int_value();
        self.builder
            .build_conditional_branch(cond_val, loop_bb, after_bb)
            .map_err(|e| e.to_string())?;

        // Execute loop body
        self.builder.position_at_end(loop_bb);
        if let Some(body_id) = children[1] {
            self.execute(body_id)?;
        }
        // Only jump back to condition if the body didn't already terminate (e.g. return)
        if self
            .builder
            .get_insert_block()
            .and_then(|bb| bb.get_terminator())
            .is_none()
        {
            self.builder
                .build_unconditional_branch(cond_bb)
                .map_err(|e| e.to_string())?;
        }

        // Continue after loop
        self.builder.position_at_end(after_bb);
        Ok(())
    }

    fn evaluate(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let node_type = self.arena[id].node_type;
        match node_type {
            NodeType::Literal => self.visit_literal(id),
            NodeType::VarExpr => self.visit_var_expr(id),
            NodeType::Assign => self.visit_assign(id),
            NodeType::Binary => self.visit_binary(id),
            NodeType::Unary => self.visit_unary(id),
            NodeType::Logical => self.visit_logical(id),
            NodeType::Array => self.visit_array(id),
            NodeType::IndexGet => self.visit_index_get(id),
            NodeType::Call => self.visit_call(id),
            NodeType::AddressOf => self.visit_addressof(id),
            NodeType::Dereference => self.visit_dereference(id),
            NodeType::AssignDeref => self.visit_assign_deref(id),
            NodeType::StructInit => self.visit_struct_init(id),
            NodeType::ExprNew => self.visit_new(id),
            NodeType::GetField => self.visit_get_field(id),
            NodeType::AssignField => self.visit_assign_field(id),
            NodeType::Grouping => {
                let child = self.arena[id].children[0]
                    .ok_or_else(|| "Grouping node has no child".to_string())?;
                self.evaluate(child)
            }
            _ => Err(format!("Unsupported evaluation node: {:?}", node_type)),
        }
    }

    fn visit_call(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let children = self.arena[id].children.clone();
        let callee_id = children[0].expect("Call missing callee");
        let callee_name = self.arena[callee_id].token.lexeme.clone();

        if self.arena[callee_id].node_type == NodeType::VarExpr && callee_name == "print" {
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
        call_param_types.push(
            self.context
                .i8_type()
                .ptr_type(inkwell::AddressSpace::from(0))
                .into(),
        );
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

    fn emit_print_call(&mut self, arg_id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
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

        let print_fn = match self.module.get_function(ext_name) {
            Some(f) => f,
            None => self.module.add_function(ext_name, fn_type, None),
        };

        self.builder
            .build_call(print_fn, &args, "")
            .map_err(|e| e.to_string())?;
        Ok(self.context.i32_type().const_int(0, false).into())
    }

    fn visit_extern_fn(&mut self, id: NodeId) -> Result<(), String> {
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

    fn visit_function(&mut self, id: NodeId) -> Result<(), String> {
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

    fn visit_return(&mut self, id: NodeId) -> Result<(), String> {
        let children = self.arena[id].children.clone();
        let current_fn = self
            .builder
            .get_insert_block()
            .ok_or("Not currently in a basic block")?
            .get_parent()
            .ok_or("Basic block has no parent function")?;

        let fn_returns_value = current_fn.get_type().get_return_type().is_some();

        if fn_returns_value {
            let val_id = children[0]
                .ok_or_else(|| "Missing return value for non-void function".to_string())?;
            let val = self.evaluate(val_id)?;
            self.builder
                .build_return(Some(&val))
                .map_err(|e| e.to_string())?;
        } else {
            if children[0].is_some() {
                return Err("Cannot return a value from a void function".to_string());
            }
            self.builder.build_return(None).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn visit_var_decl(&mut self, id: NodeId) -> Result<(), String> {
        let name = self.arena[id].token.lexeme.clone();
        let resolved_type = &self.arena[id].resolved_type;

        // Determine LLVM type
        let llvm_type = self.get_llvm_type(resolved_type)?;

        // Allocate memory on the stack for this variable
        let alloca = self
            .builder
            .build_alloca(llvm_type, &name)
            .map_err(|e| e.to_string())?;

        // Store it in the symbol table so we can find the pointer later
        self.symbol_table.define(&name, alloca);

        // If there's an initializer, evaluate it and store the result in the allocated pointer
        let children = self.arena[id].children.clone();
        if let Some(init_id) = children[0] {
            let init_val = self.evaluate(init_id)?;
            self.builder
                .build_store(alloca, init_val)
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    fn visit_assign(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let name = self.arena[id].token.lexeme.clone();
        let children = self.arena[id].children.clone();

        let ptr = match self.symbol_table.get(&name) {
            Some(p) => p,
            None => return Err(format!("Undefined variable in codegen: {}", name)),
        };

        let val = if let Some(val_id) = children[0] {
            self.evaluate(val_id)?
        } else {
            return Err("Assignment missing right-hand side".to_string());
        };

        self.builder
            .build_store(ptr, val)
            .map_err(|e| e.to_string())?;
        Ok(val)
    }

    fn visit_var_expr(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let name = self.arena[id].token.lexeme.clone();

        if let Some(ptr) = self.symbol_table.get(&name) {
            return Ok(self
                .builder
                .build_load(ptr, &name)
                .map_err(|e| e.to_string())?);
        }

        let callable_type = match self.arena[id].resolved_type.clone() {
            CiprType::Callable(params, ret) => (params, *ret),
            _ => return Err(format!("Undefined variable in codegen: {}", name)),
        };

        let direct_fn = self
            .module
            .get_function(&name)
            .ok_or_else(|| format!("Undefined function '{}'", name))?;

        let i8_ptr_type = self
            .context
            .i8_type()
            .ptr_type(inkwell::AddressSpace::from(0));

        let mut callable_param_types: Vec<BasicMetadataTypeEnum<'ctx>> = vec![i8_ptr_type.into()];
        for p in &callable_type.0 {
            callable_param_types.push(self.get_llvm_type(p)?.into());
        }
        let expected_sig = self.build_function_type(&callable_param_types, &callable_type.1)?;
        let expected_ptr_ty = expected_sig.ptr_type(inkwell::AddressSpace::from(0));

        let fn_ptr_i8 = if direct_fn.get_type() == expected_sig {
            let casted = self
                .builder
                .build_pointer_cast(
                    direct_fn.as_global_value().as_pointer_value(),
                    expected_ptr_ty,
                    "fnptr_cast",
                )
                .map_err(|e| e.to_string())?;
            self.builder
                .build_pointer_cast(casted, i8_ptr_type, "fnptr_to_i8")
                .map_err(|e| e.to_string())?
        } else {
            let wrapper = self.ensure_wrapper_for_function(
                &name,
                direct_fn,
                &callable_type.0,
                &callable_type.1,
            )?;
            self.builder
                .build_pointer_cast(
                    wrapper.as_global_value().as_pointer_value(),
                    i8_ptr_type,
                    "wrapper_to_i8",
                )
                .map_err(|e| e.to_string())?
        };

        let callable_ty = self.callable_llvm_type();
        let mut callable_val = callable_ty.get_undef();
        callable_val = self
            .builder
            .build_insert_value(callable_val, fn_ptr_i8, 0, "callable_fn")
            .map_err(|e| e.to_string())?
            .into_struct_value();
        callable_val = self
            .builder
            .build_insert_value(callable_val, i8_ptr_type.const_null(), 1, "callable_env")
            .map_err(|e| e.to_string())?
            .into_struct_value();

        Ok(callable_val.into())
    }

    fn visit_literal(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        match &self.arena[id].value {
            Value::Int(n) => {
                let i64_type = self.context.i64_type();
                Ok(i64_type.const_int(*n as u64, false).into())
            }
            Value::Float(n) => {
                let f64_type = self.context.f64_type();
                Ok(f64_type.const_float(*n).into())
            }
            Value::Bool(b) => {
                let bool_type = self.context.bool_type();
                let int_val = if *b { 1 } else { 0 };
                Ok(bool_type.const_int(int_val, false).into())
            }
            Value::Str(s) => {
                let i64_type = self.context.i64_type();
                let i8_ptr_type = self
                    .context
                    .i8_type()
                    .ptr_type(inkwell::AddressSpace::from(0));
                let struct_type = self
                    .context
                    .struct_type(&[i64_type.into(), i8_ptr_type.into()], false);

                let len_val = i64_type.const_int(s.len() as u64, false);
                let str_ptr = self
                    .builder
                    .build_global_string_ptr(s, "strlit")
                    .map_err(|e| e.to_string())?;

                let mut struct_val = struct_type.get_undef();
                struct_val = self
                    .builder
                    .build_insert_value(struct_val, len_val, 0, "insert_len")
                    .map_err(|e| e.to_string())?
                    .into_struct_value();
                struct_val = self
                    .builder
                    .build_insert_value(struct_val, str_ptr.as_pointer_value(), 1, "insert_ptr")
                    .map_err(|e| e.to_string())?
                    .into_struct_value();

                Ok(struct_val.into())
            }
            _ => Err(format!(
                "Unsupported literal type in Codegen: {:?}",
                self.arena[id].value
            )),
        }
    }

    fn visit_binary(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let op_type = self.arena[id].token.token_type;
        let children = self.arena[id].children.clone();

        let left_id = children[0].expect("Missing left operand");
        let right_id = children[1].expect("Missing right operand");

        let left = self.evaluate(left_id)?;
        let right = self.evaluate(right_id)?;

        let operand_type = &self.arena[left_id].resolved_type;

        match operand_type {
            CiprType::Int => {
                let left_int = left.into_int_value();
                let right_int = right.into_int_value();
                match op_type {
                    TokenType::Plus => Ok(self
                        .builder
                        .build_int_add(left_int, right_int, "addtmp")
                        .map_err(|e| e.to_string())?
                        .into()),
                    TokenType::Minus => Ok(self
                        .builder
                        .build_int_sub(left_int, right_int, "subtmp")
                        .map_err(|e| e.to_string())?
                        .into()),
                    TokenType::Star => Ok(self
                        .builder
                        .build_int_mul(left_int, right_int, "multmp")
                        .map_err(|e| e.to_string())?
                        .into()),
                    TokenType::Slash => Ok(self
                        .builder
                        .build_int_signed_div(left_int, right_int, "divtmp")
                        .map_err(|e| e.to_string())?
                        .into()),
                    TokenType::EqualEqual => Ok(self
                        .builder
                        .build_int_compare(inkwell::IntPredicate::EQ, left_int, right_int, "eqtmp")
                        .map_err(|e| e.to_string())?
                        .into()),
                    TokenType::BangEqual => Ok(self
                        .builder
                        .build_int_compare(inkwell::IntPredicate::NE, left_int, right_int, "netmp")
                        .map_err(|e| e.to_string())?
                        .into()),
                    TokenType::Less => Ok(self
                        .builder
                        .build_int_compare(inkwell::IntPredicate::SLT, left_int, right_int, "lttmp")
                        .map_err(|e| e.to_string())?
                        .into()),
                    TokenType::LessEqual => Ok(self
                        .builder
                        .build_int_compare(inkwell::IntPredicate::SLE, left_int, right_int, "letmp")
                        .map_err(|e| e.to_string())?
                        .into()),
                    TokenType::Greater => Ok(self
                        .builder
                        .build_int_compare(inkwell::IntPredicate::SGT, left_int, right_int, "gttmp")
                        .map_err(|e| e.to_string())?
                        .into()),
                    TokenType::GreaterEqual => Ok(self
                        .builder
                        .build_int_compare(inkwell::IntPredicate::SGE, left_int, right_int, "getmp")
                        .map_err(|e| e.to_string())?
                        .into()),
                    _ => Err(format!("Unsupported Int binary op: {:?}", op_type)),
                }
            }
            CiprType::Float => {
                let left_float = left.into_float_value();
                let right_float = right.into_float_value();
                match op_type {
                    TokenType::Plus => Ok(self
                        .builder
                        .build_float_add(left_float, right_float, "faddtmp")
                        .map_err(|e| e.to_string())?
                        .into()),
                    TokenType::Minus => Ok(self
                        .builder
                        .build_float_sub(left_float, right_float, "fsubtmp")
                        .map_err(|e| e.to_string())?
                        .into()),
                    TokenType::Star => Ok(self
                        .builder
                        .build_float_mul(left_float, right_float, "fmultmp")
                        .map_err(|e| e.to_string())?
                        .into()),
                    TokenType::Slash => Ok(self
                        .builder
                        .build_float_div(left_float, right_float, "fdivtmp")
                        .map_err(|e| e.to_string())?
                        .into()),
                    TokenType::EqualEqual => Ok(self
                        .builder
                        .build_float_compare(
                            inkwell::FloatPredicate::OEQ,
                            left_float,
                            right_float,
                            "eqtmp",
                        )
                        .map_err(|e| e.to_string())?
                        .into()),
                    TokenType::BangEqual => Ok(self
                        .builder
                        .build_float_compare(
                            inkwell::FloatPredicate::ONE,
                            left_float,
                            right_float,
                            "netmp",
                        )
                        .map_err(|e| e.to_string())?
                        .into()),
                    TokenType::Less => Ok(self
                        .builder
                        .build_float_compare(
                            inkwell::FloatPredicate::OLT,
                            left_float,
                            right_float,
                            "lttmp",
                        )
                        .map_err(|e| e.to_string())?
                        .into()),
                    TokenType::LessEqual => Ok(self
                        .builder
                        .build_float_compare(
                            inkwell::FloatPredicate::OLE,
                            left_float,
                            right_float,
                            "letmp",
                        )
                        .map_err(|e| e.to_string())?
                        .into()),
                    TokenType::Greater => Ok(self
                        .builder
                        .build_float_compare(
                            inkwell::FloatPredicate::OGT,
                            left_float,
                            right_float,
                            "gttmp",
                        )
                        .map_err(|e| e.to_string())?
                        .into()),
                    TokenType::GreaterEqual => Ok(self
                        .builder
                        .build_float_compare(
                            inkwell::FloatPredicate::OGE,
                            left_float,
                            right_float,
                            "getmp",
                        )
                        .map_err(|e| e.to_string())?
                        .into()),
                    _ => Err(format!("Unsupported Float binary op: {:?}", op_type)),
                }
            }
            _ => Err("Can only do binary ops on Ints and Floats for now".to_string()),
        }
    }

    fn visit_unary(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let op_type = self.arena[id].token.token_type;
        let children = self.arena[id].children.clone();

        let right_id = children[0].expect("Missing right operand");
        let right = self.evaluate(right_id)?;

        let operand_type = &self.arena[right_id].resolved_type;

        match op_type {
            TokenType::Minus => match operand_type {
                CiprType::Int => {
                    let right_int = right.into_int_value();
                    Ok(self
                        .builder
                        .build_int_neg(right_int, "negtmp")
                        .map_err(|e| e.to_string())?
                        .into())
                }
                CiprType::Float => {
                    let right_float = right.into_float_value();
                    Ok(self
                        .builder
                        .build_float_neg(right_float, "fnegtmp")
                        .map_err(|e| e.to_string())?
                        .into())
                }
                _ => Err(format!(
                    "Unsupported Unary Minus operand type: {:?}",
                    operand_type
                )),
            },
            TokenType::Bang => {
                let right_int = right.into_int_value();
                Ok(self
                    .builder
                    .build_not(right_int, "nottmp")
                    .map_err(|e| e.to_string())?
                    .into())
            }
            _ => Err(format!("Unsupported Unary op: {:?}", op_type)),
        }
    }

    fn visit_logical(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let op_type = self.arena[id].token.token_type;
        let children = self.arena[id].children.clone();

        let left_id = children[0].expect("Missing left operand");
        let right_id = children[1].expect("Missing right operand");

        let left_val = self.evaluate(left_id)?.into_int_value();
        let left_bb = self.builder.get_insert_block().ok_or("No insert block")?;

        let parent_fn = left_bb.get_parent().ok_or("No parent function")?;
        let right_bb = self.context.append_basic_block(parent_fn, "logical.right");
        let merge_bb = self.context.append_basic_block(parent_fn, "logical.merge");

        if op_type == TokenType::And {
            self.builder
                .build_conditional_branch(left_val, right_bb, merge_bb)
                .map_err(|e| e.to_string())?;
        } else {
            self.builder
                .build_conditional_branch(left_val, merge_bb, right_bb)
                .map_err(|e| e.to_string())?;
        }

        self.builder.position_at_end(right_bb);
        let right_val = self.evaluate(right_id)?.into_int_value();
        let incoming_right_bb = self.builder.get_insert_block().ok_or("No insert block")?;
        self.builder
            .build_unconditional_branch(merge_bb)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(merge_bb);
        let phi = self
            .builder
            .build_phi(self.context.bool_type(), "logical.tmp")
            .map_err(|e| e.to_string())?;

        let short_circuit_val = if op_type == TokenType::And {
            self.context.bool_type().const_int(0, false)
        } else {
            self.context.bool_type().const_int(1, false)
        };

        phi.add_incoming(&[
            (&short_circuit_val, left_bb),
            (&right_val, incoming_right_bb),
        ]);

        Ok(phi.as_basic_value())
    }

    fn get_llvm_type(
        &self,
        resolved_type: &CiprType,
    ) -> Result<inkwell::types::BasicTypeEnum<'ctx>, String> {
        match resolved_type {
            CiprType::Int => Ok(self.context.i64_type().into()),
            CiprType::Float => Ok(self.context.f64_type().into()),
            CiprType::Bool => Ok(self.context.bool_type().into()),
            CiprType::Str => {
                let i64_type = self.context.i64_type();
                let i8_ptr_type = self
                    .context
                    .i8_type()
                    .ptr_type(inkwell::AddressSpace::from(0));
                let struct_type = self
                    .context
                    .struct_type(&[i64_type.into(), i8_ptr_type.into()], false);
                Ok(struct_type.into())
            }
            CiprType::Array(elem) => {
                let inner = self.get_llvm_type(elem)?;
                let i64_type = self.context.i64_type();
                let data_ptr = inner.ptr_type(inkwell::AddressSpace::from(0));
                let struct_type = self
                    .context
                    .struct_type(&[i64_type.into(), data_ptr.into()], false);
                Ok(struct_type.into())
            }
            CiprType::Pointer(inner) => {
                let inner_llvm = self.get_llvm_type(inner)?;
                Ok(inner_llvm.ptr_type(inkwell::AddressSpace::from(0)).into())
            }
            CiprType::Struct(name) => {
                let (struct_type, _) = self
                    .struct_types
                    .get(name)
                    .ok_or_else(|| format!("Unknown struct '{}'", name))?;
                Ok((*struct_type).into())
            }
            CiprType::Callable(_, _) => Ok(self.callable_llvm_type().into()),
            CiprType::Void => Err("Void is not a first-class value type".to_string()),
            CiprType::Unknown => Err("Type was not resolved before code generation".to_string()),
        }
    }

    fn ensure_runtime_oob_function(&mut self) -> inkwell::values::FunctionValue<'ctx> {
        match self.module.get_function("cipr_runtime_oob") {
            Some(f) => f,
            None => {
                let fn_type = self.context.void_type().fn_type(
                    &[
                        self.context.i64_type().into(),
                        self.context.i64_type().into(),
                    ],
                    false,
                );
                self.module.add_function(
                    "cipr_runtime_oob",
                    fn_type,
                    Some(inkwell::module::Linkage::External),
                )
            }
        }
    }

    fn get_checked_array_element_ptr(
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

        let parent_fn = self
            .builder
            .get_insert_block()
            .ok_or("Not currently in a basic block")?
            .get_parent()
            .ok_or("Basic block has no parent function")?;

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

    fn visit_array(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
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

    fn visit_index_get(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let children = self.arena[id].children.clone();
        let target_id = children[0].expect("Missing array target");
        let index_id = children[1].expect("Missing index");

        let elem_ptr = self.get_checked_array_element_ptr(target_id, index_id, "idx_ptr")?;

        Ok(self
            .builder
            .build_load(elem_ptr, "idx_load")
            .map_err(|e| e.to_string())?)
    }

    fn get_eval_pointer(
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

                let (_, _) = self.struct_types.get(&struct_name).unwrap();
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

    fn visit_addressof(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let target_id = self.arena[id].children[0].unwrap();
        Ok(self.get_eval_pointer(target_id)?.into())
    }

    fn get_struct_field_index(&self, struct_name: &str, field_name: &str) -> Result<u32, String> {
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

    fn visit_struct_init(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let cipr_type = self.arena[id].resolved_type.clone();
        let struct_name = match cipr_type {
            CiprType::Struct(name) => name,
            _ => return Err("StructInit resolved to non-struct type".to_string()),
        };

        let (struct_type, _) = self.struct_types.get(&struct_name).unwrap();
        let mut struct_val = struct_type.const_zero();

        for (i, child_opt) in self.arena[id].children.iter().enumerate() {
            let child_id = child_opt.unwrap();
            let assign_node = &self.arena[child_id];
            let val_id = assign_node.children[0].unwrap();
            let val = self.evaluate(val_id)?;
            struct_val = self
                .builder
                .build_insert_value(struct_val, val, i as u32, "struct_init")
                .map_err(|e| e.to_string())?
                .into_struct_value();
        }

        Ok(struct_val.into())
    }

    fn visit_new(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let struct_name = self.arena[id].token.lexeme.clone();
        let (struct_type, _) = self.struct_types.get(&struct_name).unwrap();

        let alloc_size_bytes = struct_type.size_of().unwrap();

        // Auto-declare cipr_malloc if not yet in module (i8* cipr_malloc(i64))
        let malloc_fn = match self.module.get_function("cipr_malloc") {
            Some(f) => f,
            None => {
                let i8_ptr = self
                    .context
                    .i8_type()
                    .ptr_type(inkwell::AddressSpace::from(0));
                let fn_type = i8_ptr.fn_type(&[self.context.i64_type().into()], false);
                self.module.add_function(
                    "cipr_malloc",
                    fn_type,
                    Some(inkwell::module::Linkage::External),
                )
            }
        };

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
            .unwrap();

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

    fn visit_delete(&mut self, id: NodeId) -> Result<(), String> {
        let child_id = self.arena[id].children[0].unwrap();
        let val_ptr = self.evaluate(child_id)?.into_pointer_value();

        let i8_ptr_type = self
            .context
            .i8_type()
            .ptr_type(inkwell::AddressSpace::from(0));
        let raw_ptr = self
            .builder
            .build_pointer_cast(val_ptr, i8_ptr_type, "delete_ptr_cast")
            .unwrap();

        // Auto-declare cipr_free if not yet in module (void cipr_free(i8*))
        let free_fn = match self.module.get_function("cipr_free") {
            Some(f) => f,
            None => {
                let fn_type = self
                    .context
                    .void_type()
                    .fn_type(&[i8_ptr_type.into()], false);
                self.module.add_function(
                    "cipr_free",
                    fn_type,
                    Some(inkwell::module::Linkage::External),
                )
            }
        };
        self.builder
            .build_call(free_fn, &[raw_ptr.into()], "free_call")
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn visit_get_field(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let target_id = self.arena[id].children[0].unwrap();
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
            let (_, _) = self.struct_types.get(&struct_name).unwrap();
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

    fn visit_assign_field(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let target_expr_id = self.arena[id].children[0].unwrap();
        let val_expr_id = self.arena[id].children[1].unwrap();

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

        let (_, _) = self.struct_types.get(&struct_name).unwrap();
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

    fn visit_dereference(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let children = self.arena[id].children.clone();
        let target_id = children[0].expect("Dereference missing target");
        let ptr_val = self.evaluate(target_id)?.into_pointer_value();
        Ok(self
            .builder
            .build_load(ptr_val, "deref_load")
            .map_err(|e| e.to_string())?)
    }

    fn visit_assign_deref(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
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
