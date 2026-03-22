use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::{BasicMetadataTypeEnum, BasicType};
use inkwell::values::BasicValueEnum;

use crate::ast::{CiprType, NodeArena, NodeId, NodeType};
use crate::symbol_table::{SymbolTable, SymbolTableRef};
use crate::token::{TokenType, Value};

pub struct Codegen<'a, 'ctx> {
    pub context: &'ctx Context,
    pub builder: &'a Builder<'ctx>,
    pub module: &'a Module<'ctx>,
    pub arena: &'a NodeArena,
    pub symbol_table: SymbolTableRef<'ctx>,
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
        }
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
        self.builder.build_return(Some(&zero)).map_err(|e| e.to_string())?;

        Ok(())
    }

    fn execute(&mut self, id: NodeId) -> Result<(), String> {
        let node_type = self.arena[id].node_type;

        match node_type {
            NodeType::StmtList | NodeType::StmtBlock => {
                let prev_env = std::rc::Rc::clone(&self.symbol_table);
                self.symbol_table = SymbolTable::with_enclosing(&prev_env);

                let children = self.arena[id].children.clone();
                for c in children.iter().flatten() {
                    self.execute(*c)?;
                }

                self.symbol_table = prev_env;
                Ok(())
            }
            NodeType::StmtVarDecl => self.visit_var_decl(id),
            NodeType::StmtExpr => {
                let children = self.arena[id].children.clone();
                if let Some(expr_id) = children[0] {
                    self.evaluate(expr_id)?;
                }
                Ok(())
            }
            NodeType::StmtIf => self.visit_if(id),
            NodeType::StmtWhile => self.visit_while(id),
            NodeType::StmtFunction => self.visit_function(id),
            NodeType::StmtReturn => self.visit_return(id),
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
        self.builder
            .build_unconditional_branch(merge_bb)
            .map_err(|e| e.to_string())?;

        // Else block
        self.builder.position_at_end(else_bb);
        if has_else {
            if let Some(else_id) = children[2] {
                self.execute(else_id)?;
            }
        }
        self.builder
            .build_unconditional_branch(merge_bb)
            .map_err(|e| e.to_string())?;

        // Continue
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
        // Jump back to condition
        self.builder
            .build_unconditional_branch(cond_bb)
            .map_err(|e| e.to_string())?;

        // Continue
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
            _ => Err(format!("Unsupported evaluation node: {:?}", node_type)),
        }
    }

    fn visit_call(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let children = self.arena[id].children.clone();
        let callee_id = children[0].expect("Call missing callee");
        let callee_name = self.arena[callee_id].token.lexeme.clone();

        if callee_name == "print" {
            let arg_id = children[1].expect("Print missing argument");
            let arg_val = self.evaluate(arg_id)?;
            let arg_type = &self.arena[arg_id].resolved_type;

            let (ext_name, fn_type, args) = match arg_type {
                CiprType::Str => {
                    let struct_type = self.get_llvm_type(&CiprType::Str)?;
                    let ftype = self.context.void_type().fn_type(&[struct_type.into()], false);
                    ("cipr_print_str", ftype, vec![arg_val.into()])
                }
                CiprType::Int => {
                    let ftype = self.context.void_type().fn_type(&[self.context.i64_type().into()], false);
                    ("cipr_print_int", ftype, vec![arg_val.into()])
                }
                CiprType::Float => {
                    let ftype = self.context.void_type().fn_type(&[self.context.f64_type().into()], false);
                    ("cipr_print_float", ftype, vec![arg_val.into()])
                }
                CiprType::Bool => {
                    let ftype = self.context.void_type().fn_type(&[self.context.bool_type().into()], false);
                    ("cipr_print_bool", ftype, vec![arg_val.into()])
                }
                _ => return Err(format!("Unsupported type for print: {:?}", arg_type)),
            };

            let print_fn = match self.module.get_function(ext_name) {
                Some(f) => f,
                None => self.module.add_function(ext_name, fn_type, None),
            };

            self.builder.build_call(print_fn, &args, "").map_err(|e| e.to_string())?;
            return Ok(self.context.i32_type().const_int(0, false).into());
        }

        if callee_name == "time" {
            let ext_name = "cipr_time";
            let ftype = self.context.f64_type().fn_type(&[], false);
            let time_fn = match self.module.get_function(ext_name) {
                Some(f) => f,
                None => self.module.add_function(ext_name, ftype, None),
            };

            let call_site = self.builder.build_call(time_fn, &[], "timetmp").map_err(|e| e.to_string())?;
            return match call_site.try_as_basic_value() {
                inkwell::values::ValueKind::Basic(v) => Ok(v),
                _ => return Err("cipr_time did not return basic value".to_string())
            };
        }

        // Handle user-defined function calls
        let function = self
            .module
            .get_function(&callee_name)
            .ok_or_else(|| format!("Undefined function: {}", callee_name))?;

        let mut args = Vec::new();
        for i in 1..children.len() {
            if let Some(arg_id) = children[i] {
                args.push(self.evaluate(arg_id)?.into());
            }
        }

        let call_site = self
            .builder
            .build_call(function, &args, &format!("{}_call", callee_name))
            .map_err(|e| e.to_string())?;

        match call_site.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(v) => Ok(v),
            inkwell::values::ValueKind::Instruction(_) => {
                // If it returns void, return a dummy i32 0 for now
                Ok(self.context.i32_type().const_int(0, false).into())
            }
        }
    }

    fn visit_function(&mut self, id: NodeId) -> Result<(), String> {
        let name = self.arena[id].token.lexeme.clone();
        let children = self.arena[id].children.clone();
        let param_count = children.len() - 1;

        // Determine return type
        let ret_type = self.get_llvm_type(&self.arena[id].resolved_type)
            .unwrap_or_else(|_| self.context.i32_type().into());

        // Determine parameter types
        let mut param_types = Vec::new();
        for i in 0..param_count {
            if let Some(param_id) = children[i] {
                let p_type: BasicMetadataTypeEnum = self.get_llvm_type(&self.arena[param_id].resolved_type)?.into();
                param_types.push(p_type);
            }
        }

        let fn_type = match ret_type {
            inkwell::types::BasicTypeEnum::IntType(t) => t.fn_type(&param_types, false),
            inkwell::types::BasicTypeEnum::FloatType(t) => t.fn_type(&param_types, false),
            _ => self.context.i32_type().fn_type(&param_types, false),
        };
        let function = self.module.add_function(&name, fn_type, None);

        // Save current insertion point
        let original_bb = self.builder.get_insert_block();

        // Create entry block
        let entry_bb = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry_bb);

        // New scope for function body
        let prev_env = std::rc::Rc::clone(&self.symbol_table);
        self.symbol_table = SymbolTable::with_enclosing(&prev_env);

        // Allocate and store parameters
        for (i, arg) in function.get_param_iter().enumerate() {
            if let Some(param_id) = children[i] {
                let p_name = self.arena[param_id].token.lexeme.clone();
                let p_type = self.get_llvm_type(&self.arena[param_id].resolved_type)?;
                let alloca = self
                    .builder
                    .build_alloca(p_type, &p_name)
                    .map_err(|e| e.to_string())?;
                self.builder.build_store(alloca, arg).map_err(|e| e.to_string())?;
                self.symbol_table.borrow_mut().define(&p_name, alloca);
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
            let zero = self.context.i32_type().const_int(0, false);
            self.builder
                .build_return(Some(&zero))
                .map_err(|e| e.to_string())?;
        }

        // Restore scope and insertion point
        self.symbol_table = prev_env;
        if let Some(bb) = original_bb {
            self.builder.position_at_end(bb);
        }

        Ok(())
    }

    fn visit_return(&mut self, id: NodeId) -> Result<(), String> {
        let children = self.arena[id].children.clone();
        if let Some(val_id) = children[0] {
            let val = self.evaluate(val_id)?;
            self.builder
                .build_return(Some(&val))
                .map_err(|e| e.to_string())?;
        } else {
            // Implicitly return 0 for void functions
            let zero = self.context.i32_type().const_int(0, false);
            self.builder
                .build_return(Some(&zero))
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn visit_var_decl(&mut self, id: NodeId) -> Result<(), String> {
        let name = self.arena[id].token.lexeme.clone();
        let resolved_type = &self.arena[id].resolved_type;

        // Determine LLVM type
        let llvm_type = self.get_llvm_type(resolved_type)?;

        // Allocate memory on the stack for this variable
        let alloca = self.builder.build_alloca(llvm_type, &name).map_err(|e| e.to_string())?;

        // Store it in the symbol table so we can find the pointer later
        self.symbol_table.borrow_mut().define(&name, alloca);

        // If there's an initializer, evaluate it and store the result in the allocated pointer
        let children = self.arena[id].children.clone();
        if let Some(init_id) = children[0] {
            let init_val = self.evaluate(init_id)?;
            self.builder.build_store(alloca, init_val).map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    fn visit_assign(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let name = self.arena[id].token.lexeme.clone();
        let children = self.arena[id].children.clone();

        let ptr = match self.symbol_table.borrow().get(&name) {
            Some(p) => p,
            None => return Err(format!("Undefined variable in codegen: {}", name)),
        };

        let val = if let Some(val_id) = children[0] {
            self.evaluate(val_id)?
        } else {
            return Err("Assignment missing right-hand side".to_string());
        };

        self.builder.build_store(ptr, val).map_err(|e| e.to_string())?;
        Ok(val)
    }

    fn visit_var_expr(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let name = self.arena[id].token.lexeme.clone();

        let ptr = match self.symbol_table.borrow().get(&name) {
            Some(p) => p,
            None => return Err(format!("Undefined variable in codegen: {}", name)),
        };

        // Load the value out of the memory address (Inkwell 0.8 build_load takes ptr and name)
        Ok(self.builder.build_load(ptr, &name).map_err(|e| e.to_string())?)
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
                let i8_ptr_type = self.context.i8_type().ptr_type(inkwell::AddressSpace::from(0));
                let struct_type = self.context.struct_type(&[i64_type.into(), i8_ptr_type.into()], false);
                
                let len_val = i64_type.const_int(s.len() as u64, false);
                let str_ptr = self.builder.build_global_string_ptr(s, "strlit").map_err(|e| e.to_string())?;

                let mut struct_val = struct_type.get_undef();
                struct_val = self.builder.build_insert_value(struct_val, len_val, 0, "insert_len").map_err(|e| e.to_string())?.into_struct_value();
                struct_val = self.builder.build_insert_value(struct_val, str_ptr.as_pointer_value(), 1, "insert_ptr").map_err(|e| e.to_string())?.into_struct_value();

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
                    TokenType::Plus => Ok(self.builder.build_int_add(left_int, right_int, "addtmp").map_err(|e| e.to_string())?.into()),
                    TokenType::Minus => Ok(self.builder.build_int_sub(left_int, right_int, "subtmp").map_err(|e| e.to_string())?.into()),
                    TokenType::Star => Ok(self.builder.build_int_mul(left_int, right_int, "multmp").map_err(|e| e.to_string())?.into()),
                    TokenType::Slash => Ok(self.builder.build_int_signed_div(left_int, right_int, "divtmp").map_err(|e| e.to_string())?.into()),
                    TokenType::EqualEqual => Ok(self.builder.build_int_compare(inkwell::IntPredicate::EQ, left_int, right_int, "eqtmp").map_err(|e| e.to_string())?.into()),
                    TokenType::BangEqual => Ok(self.builder.build_int_compare(inkwell::IntPredicate::NE, left_int, right_int, "netmp").map_err(|e| e.to_string())?.into()),
                    TokenType::Less => Ok(self.builder.build_int_compare(inkwell::IntPredicate::SLT, left_int, right_int, "lttmp").map_err(|e| e.to_string())?.into()),
                    TokenType::LessEqual => Ok(self.builder.build_int_compare(inkwell::IntPredicate::SLE, left_int, right_int, "letmp").map_err(|e| e.to_string())?.into()),
                    TokenType::Greater => Ok(self.builder.build_int_compare(inkwell::IntPredicate::SGT, left_int, right_int, "gttmp").map_err(|e| e.to_string())?.into()),
                    TokenType::GreaterEqual => Ok(self.builder.build_int_compare(inkwell::IntPredicate::SGE, left_int, right_int, "getmp").map_err(|e| e.to_string())?.into()),
                    _ => Err(format!("Unsupported Int binary op: {:?}", op_type)),
                }
            }
            CiprType::Float => {
                let left_float = left.into_float_value();
                let right_float = right.into_float_value();
                match op_type {
                    TokenType::Plus => Ok(self.builder.build_float_add(left_float, right_float, "faddtmp").map_err(|e| e.to_string())?.into()),
                    TokenType::Minus => Ok(self.builder.build_float_sub(left_float, right_float, "fsubtmp").map_err(|e| e.to_string())?.into()),
                    TokenType::Star => Ok(self.builder.build_float_mul(left_float, right_float, "fmultmp").map_err(|e| e.to_string())?.into()),
                    TokenType::Slash => Ok(self.builder.build_float_div(left_float, right_float, "fdivtmp").map_err(|e| e.to_string())?.into()),
                    TokenType::EqualEqual => Ok(self.builder.build_float_compare(inkwell::FloatPredicate::OEQ, left_float, right_float, "eqtmp").map_err(|e| e.to_string())?.into()),
                    TokenType::BangEqual => Ok(self.builder.build_float_compare(inkwell::FloatPredicate::ONE, left_float, right_float, "netmp").map_err(|e| e.to_string())?.into()),
                    TokenType::Less => Ok(self.builder.build_float_compare(inkwell::FloatPredicate::OLT, left_float, right_float, "lttmp").map_err(|e| e.to_string())?.into()),
                    TokenType::LessEqual => Ok(self.builder.build_float_compare(inkwell::FloatPredicate::OLE, left_float, right_float, "letmp").map_err(|e| e.to_string())?.into()),
                    TokenType::Greater => Ok(self.builder.build_float_compare(inkwell::FloatPredicate::OGT, left_float, right_float, "gttmp").map_err(|e| e.to_string())?.into()),
                    TokenType::GreaterEqual => Ok(self.builder.build_float_compare(inkwell::FloatPredicate::OGE, left_float, right_float, "getmp").map_err(|e| e.to_string())?.into()),
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
            TokenType::Minus => {
                match operand_type {
                    CiprType::Int => {
                        let right_int = right.into_int_value();
                        Ok(self.builder.build_int_neg(right_int, "negtmp").map_err(|e| e.to_string())?.into())
                    }
                    CiprType::Float => {
                        let right_float = right.into_float_value();
                        Ok(self.builder.build_float_neg(right_float, "fnegtmp").map_err(|e| e.to_string())?.into())
                    }
                    _ => Err(format!("Unsupported Unary Minus operand type: {:?}", operand_type)),
                }
            }
            TokenType::Bang => {
                let right_int = right.into_int_value();
                Ok(self.builder.build_not(right_int, "nottmp").map_err(|e| e.to_string())?.into())
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
            self.builder.build_conditional_branch(left_val, right_bb, merge_bb).map_err(|e| e.to_string())?;
        } else {
            self.builder.build_conditional_branch(left_val, merge_bb, right_bb).map_err(|e| e.to_string())?;
        }

        self.builder.position_at_end(right_bb);
        let right_val = self.evaluate(right_id)?.into_int_value();
        let incoming_right_bb = self.builder.get_insert_block().ok_or("No insert block")?;
        self.builder.build_unconditional_branch(merge_bb).map_err(|e| e.to_string())?;

        self.builder.position_at_end(merge_bb);
        let phi = self.builder.build_phi(self.context.bool_type(), "logical.tmp").map_err(|e| e.to_string())?;
        
        let short_circuit_val = if op_type == TokenType::And {
            self.context.bool_type().const_int(0, false)
        } else {
            self.context.bool_type().const_int(1, false)
        };

        phi.add_incoming(&[
            (&short_circuit_val, left_bb),
            (&right_val, incoming_right_bb)
        ]);

        Ok(phi.as_basic_value())
    }

    fn get_llvm_type(&self, resolved_type: &CiprType) -> Result<inkwell::types::BasicTypeEnum<'ctx>, String> {
        match resolved_type {
            CiprType::Int => Ok(self.context.i64_type().into()),
            CiprType::Float => Ok(self.context.f64_type().into()),
            CiprType::Bool => Ok(self.context.bool_type().into()),
            CiprType::Str => {
                let i64_type = self.context.i64_type();
                let i8_ptr_type = self.context.i8_type().ptr_type(inkwell::AddressSpace::from(0));
                let struct_type = self.context.struct_type(&[i64_type.into(), i8_ptr_type.into()], false);
                Ok(struct_type.into())
            }
            CiprType::Array(elem) => {
                let inner = self.get_llvm_type(elem)?;
                Ok(inner.ptr_type(inkwell::AddressSpace::from(0)).into())
            }
            CiprType::Void | CiprType::Unknown => Ok(self.context.i32_type().into()), // Dummy type for void
            _ => Err(format!("Unsupported LLVM type mapping for: {:?}", resolved_type)),
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
        let ptr = self.builder.build_array_alloca(elem_type, size_val, "array_alloc").map_err(|e| e.to_string())?;

        for (i, child_opt) in children.iter().enumerate() {
            if let Some(child_id) = child_opt {
                let val = self.evaluate(*child_id)?;
                let idx_val = self.context.i64_type().const_int(i as u64, false);
                let elem_ptr = unsafe {
                    self.builder.build_in_bounds_gep(ptr, &[idx_val], "elem_ptr").map_err(|e| e.to_string())?
                };
                self.builder.build_store(elem_ptr, val).map_err(|e| e.to_string())?;
            }
        }

        Ok(ptr.into())
    }

    fn visit_index_get(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let children = self.arena[id].children.clone();
        let target_id = children[0].expect("Missing array target");
        let index_id = children[1].expect("Missing index");

        let target_ptr = self.evaluate(target_id)?.into_pointer_value();
        let index_val = self.evaluate(index_id)?.into_int_value();

        let elem_ptr = unsafe {
            self.builder.build_in_bounds_gep(target_ptr, &[index_val], "idx_ptr").map_err(|e| e.to_string())?
        };
        
        Ok(self.builder.build_load(elem_ptr, "idx_load").map_err(|e| e.to_string())?)
    }
}
