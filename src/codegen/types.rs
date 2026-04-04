use crate::ast::CiprType;
use crate::codegen::Codegen;
use inkwell::types::{BasicMetadataTypeEnum, BasicType};

impl<'a, 'ctx> Codegen<'a, 'ctx> {
    pub(crate) fn callable_llvm_type(&self) -> inkwell::types::StructType<'ctx> {
        let i8_ptr_type = self.i8_ptr_type();
        self.context
            .struct_type(&[i8_ptr_type.into(), i8_ptr_type.into()], false)
    }

    pub(crate) fn build_function_type(
        &self,
        param_types: &[BasicMetadataTypeEnum<'ctx>],
        ret_type: &CiprType,
    ) -> Result<inkwell::types::FunctionType<'ctx>, String> {
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

    pub(crate) fn get_llvm_type(
        &self,
        resolved_type: &CiprType,
    ) -> Result<inkwell::types::BasicTypeEnum<'ctx>, String> {
        match resolved_type {
            CiprType::Int => Ok(self.context.i64_type().into()),
            CiprType::Float => Ok(self.context.f64_type().into()),
            CiprType::Bool => Ok(self.context.bool_type().into()),
            CiprType::Str => {
                let i64_type = self.context.i64_type();
                let i8_ptr_type = self.i8_ptr_type();
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
            CiprType::Null => Err("Null is not a first-class value type".to_string()),
            CiprType::Void => Err("Void is not a first-class value type".to_string()),
            CiprType::Unknown => Err("Type was not resolved before code generation".to_string()),
        }
    }
}
