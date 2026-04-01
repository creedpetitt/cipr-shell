pub mod arrays;
pub mod calls;
pub mod control_flow;
pub mod env;
pub mod expr;
pub mod pointers;
pub mod structs;
pub mod vars;

use std::collections::HashMap;

use crate::ast::{CiprType, NodeArena, NodeId, NodeType};
use crate::type_syntax;
use env::TypeEnv;

pub struct TypeChecker<'a> {
    pub arena: &'a mut NodeArena,
    pub env: TypeEnv,
    pub had_error: bool,
    current_return_type: Option<CiprType>,
    pub structs: HashMap<String, Vec<(String, CiprType)>>,
}

impl<'a> TypeChecker<'a> {
    pub fn new(arena: &'a mut NodeArena) -> Self {
        let mut env = TypeEnv::new();
        env.define(
            "print",
            CiprType::Callable(vec![CiprType::Unknown], Box::new(CiprType::Void)),
        );

        Self {
            arena,
            env,
            had_error: false,
            current_return_type: None,
            structs: HashMap::new(),
        }
    }

    pub fn error(&mut self, line: usize, message: &str) {
        eprintln!("[line {}] Type Error: {}", line, message);
        self.had_error = true;
    }

    pub fn parse_type_annotation(annotation: &Option<CiprType>) -> CiprType {
        type_syntax::parse_annotation(annotation)
    }

    pub(crate) fn check_child(&mut self, child_opt: Option<NodeId>) -> CiprType {
        if let Some(id) = child_opt {
            self.check(id)
        } else {
            CiprType::Unknown
        }
    }

    pub fn check(&mut self, id: NodeId) -> CiprType {
        let node_type = self.arena[id].node_type;

        let t = match node_type {
            NodeType::StmtList => self.check_block(id),
            NodeType::StmtBlock => self.check_block_stmt(id),
            NodeType::StmtVarDecl => self.check_var_decl(id),
            NodeType::StmtFunction => self.check_function(id),
            NodeType::StmtExpr => self.check_expr_stmt(id),
            NodeType::StmtIf => self.check_if(id),
            NodeType::StmtWhile => self.check_while(id),
            NodeType::StmtReturn => self.check_return(id),
            NodeType::Literal => self.check_literal(id),
            NodeType::VarExpr => self.check_var_expr(id),
            NodeType::Assign => self.check_assign(id),
            NodeType::Binary => self.check_binary(id),
            NodeType::Unary => self.check_unary(id),
            NodeType::Logical => self.check_logical(id),
            NodeType::Call => self.check_call(id),
            NodeType::Array => self.check_array(id),
            NodeType::IndexGet => self.check_index_get(id),
            NodeType::AddressOf => self.check_addressof(id),
            NodeType::Dereference => self.check_dereference(id),
            NodeType::AssignDeref => self.check_assign_deref(id),
            NodeType::StmtStructDecl => self.check_struct_decl(id),
            NodeType::StructInit => self.check_struct_init(id),
            NodeType::GetField => self.check_get_field(id),
            NodeType::AssignField => self.check_assign_field(id),
            NodeType::StmtExternFn => self.check_extern_fn(id),
            NodeType::StmtInclude => self.check_include(id),
            NodeType::ExprNew => self.check_new(id),
            NodeType::StmtDelete => self.check_delete(id),
            NodeType::Grouping => {
                let child = self.arena[id].children[0].expect("Grouping node has no child");
                self.check(child)
            }
        };

        self.arena[id].resolved_type = t.clone();
        t
    }
}
