use crate::ast::{CiprType, NodeId};
use crate::codegen::Codegen;
use crate::token::{TokenType, Value};
use inkwell::types::{BasicMetadataTypeEnum, BasicType};
use inkwell::values::BasicValueEnum;

impl<'a, 'ctx> Codegen<'a, 'ctx> {
    pub(crate) fn visit_var_decl(&mut self, id: NodeId) -> Result<(), String> {
        let name = self.arena[id].token.lexeme.clone();
        let resolved_type = &self.arena[id].resolved_type;

        let llvm_type = self.get_llvm_type(resolved_type)?;

        let alloca = self
            .builder
            .build_alloca(llvm_type, &name)
            .map_err(|e| e.to_string())?;

        self.symbol_table.define(&name, alloca);

        let children = self.arena[id].children.clone();
        if let Some(init_id) = children[0] {
            let init_val = self.evaluate(init_id)?;
            self.builder
                .build_store(alloca, init_val)
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    pub(crate) fn visit_assign(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
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

    pub(crate) fn visit_var_expr(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
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

        let i8_ptr_type = self.i8_ptr_type();

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

    pub(crate) fn visit_literal(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
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
                let i8_ptr_type = self.i8_ptr_type();
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
            Value::Null => match &self.arena[id].resolved_type {
                CiprType::Pointer(inner) => {
                    let inner_type = self.get_llvm_type(inner)?;
                    Ok(inner_type
                        .ptr_type(inkwell::AddressSpace::from(0))
                        .const_null()
                        .into())
                }
                other => Err(format!(
                    "Null literal must resolve to a pointer type before code generation, got {:?}",
                    other
                )),
            },
        }
    }

    pub(crate) fn visit_binary(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
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
            CiprType::Bool => {
                let left_bool = left.into_int_value();
                let right_bool = right.into_int_value();
                match op_type {
                    TokenType::EqualEqual => Ok(self
                        .builder
                        .build_int_compare(
                            inkwell::IntPredicate::EQ,
                            left_bool,
                            right_bool,
                            "beqtmp",
                        )
                        .map_err(|e| e.to_string())?
                        .into()),
                    TokenType::BangEqual => Ok(self
                        .builder
                        .build_int_compare(
                            inkwell::IntPredicate::NE,
                            left_bool,
                            right_bool,
                            "bnetmp",
                        )
                        .map_err(|e| e.to_string())?
                        .into()),
                    _ => Err(format!("Unsupported Bool binary op: {:?}", op_type)),
                }
            }
            CiprType::Pointer(_) => {
                let left_ptr = left.into_pointer_value();
                let right_ptr = right.into_pointer_value();
                let intptr_type = self.context.i64_type();
                let left_int = self
                    .builder
                    .build_ptr_to_int(left_ptr, intptr_type, "ptrcmp_left")
                    .map_err(|e| e.to_string())?;
                let right_int = self
                    .builder
                    .build_ptr_to_int(right_ptr, intptr_type, "ptrcmp_right")
                    .map_err(|e| e.to_string())?;
                match op_type {
                    TokenType::EqualEqual => Ok(self
                        .builder
                        .build_int_compare(inkwell::IntPredicate::EQ, left_int, right_int, "peqtmp")
                        .map_err(|e| e.to_string())?
                        .into()),
                    TokenType::BangEqual => Ok(self
                        .builder
                        .build_int_compare(inkwell::IntPredicate::NE, left_int, right_int, "pnetmp")
                        .map_err(|e| e.to_string())?
                        .into()),
                    _ => Err(format!("Unsupported Pointer binary op: {:?}", op_type)),
                }
            }
            _ => Err("Unsupported operand type for binary operation".to_string()),
        }
    }

    pub(crate) fn visit_unary(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
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
}
