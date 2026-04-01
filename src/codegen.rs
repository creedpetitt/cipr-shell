pub mod arrays;
pub mod calls;
pub mod control_flow;
pub mod heap;
pub mod pointers;
pub mod structs;
pub mod types;
pub mod vars;

use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::values::BasicValueEnum;

use crate::ast::{NodeArena, NodeId, NodeType};
use crate::symbol_table::SymbolTable;

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

    pub fn i8_ptr_type(&self) -> inkwell::types::PointerType<'ctx> {
        self.context.i8_type().ptr_type(inkwell::AddressSpace::from(0))
    }

    pub fn current_function(&self) -> Result<inkwell::values::FunctionValue<'ctx>, String> {
        self.builder
            .get_insert_block()
            .ok_or("Not currently in a basic block".to_string())?
            .get_parent()
            .ok_or("Basic block has no parent function".to_string())
    }

    pub fn get_or_add_function(
        &self,
        name: &str,
        fn_type: inkwell::types::FunctionType<'ctx>,
    ) -> inkwell::values::FunctionValue<'ctx> {
        match self.module.get_function(name) {
            Some(f) => f,
            None => self.module.add_function(name, fn_type, Some(inkwell::module::Linkage::External)),
        }
    }

    pub fn compile(&mut self, root_id: NodeId) -> Result<(), String> {
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

        let zero = self.context.i32_type().const_int(0, false);
        self.builder
            .build_return(Some(&zero))
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn execute(&mut self, id: NodeId) -> Result<(), String> {
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
            NodeType::StmtStructDecl => self.visit_struct_decl(id),
            _ => Ok(()),
        }
    }

    pub fn evaluate(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
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
}
