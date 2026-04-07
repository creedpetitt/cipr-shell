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
use crate::diagnostics::{DiagnosticPhase, Diagnostics};
use crate::type_syntax;
use env::TypeEnv;

pub struct TypeChecker<'a> {
    pub arena: &'a mut NodeArena,
    pub env: TypeEnv,
    pub had_error: bool,
    current_return_type: Option<CiprType>,
    pub structs: HashMap<String, Vec<(String, CiprType)>>,
    source_name: String,
    diagnostics: Diagnostics,
}

impl<'a> TypeChecker<'a> {
    pub fn new(arena: &'a mut NodeArena, source_name: &str) -> Self {
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
            source_name: source_name.to_string(),
            diagnostics: Diagnostics::new(),
        }
    }

    pub fn take_diagnostics(&mut self) -> Diagnostics {
        std::mem::take(&mut self.diagnostics)
    }

    pub fn error(&mut self, line: usize, message: &str) {
        self.diagnostics
            .emit_line(DiagnosticPhase::Type, &self.source_name, line, message);
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

    pub(crate) fn predeclare_structs(&mut self, children: &[Option<NodeId>]) {
        for child in children.iter().flatten() {
            if self.arena[*child].node_type == NodeType::StmtStructDecl {
                if !self.register_struct_decl(*child) {
                    let name = self.arena[*child].token.lexeme.clone();
                    let line = self.arena[*child].token.line;
                    self.error(line, &format!("Duplicate struct declaration '{}'.", name));
                }
            }
        }
    }

    pub(crate) fn register_struct_decl(&mut self, id: NodeId) -> bool {
        let name = self.arena[id].token.lexeme.clone();
        if self.structs.contains_key(&name) {
            return false;
        }

        let mut fields = Vec::new();
        let children = self.arena[id].children.clone();
        for child_opt in &children {
            if let Some(child_id) = child_opt {
                let field_node = &self.arena[*child_id];
                let field_name = field_node.token.lexeme.clone();
                let field_type = Self::parse_type_annotation(&field_node.type_annotation);
                fields.push((field_name, field_type));
            }
        }
        self.structs.insert(name, fields);
        true
    }

    pub(crate) fn validate_type(&mut self, ty: &CiprType, line: usize) {
        match ty {
            CiprType::Array(inner) => self.validate_type(inner, line),
            CiprType::Pointer(inner) => {
                if **inner == CiprType::Void {
                    self.error(line, "Pointers to void are not supported.");
                }
                self.validate_type(inner, line);
            }
            CiprType::Callable(params, ret) => {
                for param in params {
                    self.validate_type(param, line);
                }
                self.validate_type(ret, line);
            }
            CiprType::Struct(name) => {
                if !self.structs.contains_key(name) {
                    self.error(line, &format!("Undefined type '{}'.", name));
                }
            }
            CiprType::Int
            | CiprType::Float
            | CiprType::Str
            | CiprType::Bool
            | CiprType::Null
            | CiprType::Void
            | CiprType::Unknown => {}
        }
    }

    fn enforce_value_type_rules(&mut self, ty: &CiprType, line: usize, context: &str) {
        match ty {
            CiprType::Void => self.error(line, &format!("{} cannot be void.", context)),
            CiprType::Array(inner) => {
                self.enforce_value_type_rules(inner, line, "Array element type")
            }
            CiprType::Pointer(inner) => {
                if **inner != CiprType::Void {
                    self.enforce_value_type_rules(inner, line, "Pointer target type");
                }
            }
            CiprType::Callable(params, ret) => {
                for param in params {
                    self.enforce_value_type_rules(param, line, "Function parameter type");
                }
                self.enforce_return_type_rules(ret, line, "Function return type");
            }
            CiprType::Int
            | CiprType::Float
            | CiprType::Str
            | CiprType::Bool
            | CiprType::Null
            | CiprType::Struct(_)
            | CiprType::Unknown => {}
        }
    }

    fn enforce_return_type_rules(&mut self, ty: &CiprType, line: usize, context: &str) {
        match ty {
            CiprType::Array(_) => {
                self.error(
                    line,
                    &format!(
                        "{} cannot be an array. Arrays are stack-allocated and cannot be returned.",
                        context
                    ),
                );
            }
            _ if *ty != CiprType::Void => {
                self.enforce_value_type_rules(ty, line, context);
            }
            _ => {}
        }
    }

    pub(crate) fn validate_value_type(&mut self, ty: &CiprType, line: usize, context: &str) {
        self.validate_type(ty, line);
        self.enforce_value_type_rules(ty, line, context);
    }

    pub(crate) fn validate_return_type(&mut self, ty: &CiprType, line: usize, context: &str) {
        self.validate_type(ty, line);
        self.enforce_return_type_rules(ty, line, context);
    }

    pub(crate) fn coerce_null_child(
        &mut self,
        child_opt: Option<NodeId>,
        expected_type: &CiprType,
    ) -> bool {
        let Some(child_id) = child_opt else {
            return false;
        };

        if self.arena[child_id].resolved_type != CiprType::Null {
            return false;
        }

        if matches!(expected_type, CiprType::Pointer(_)) {
            self.arena[child_id].resolved_type = expected_type.clone();
            if self.arena[child_id].node_type == NodeType::Grouping {
                let nested = self.arena[child_id].children[0];
                self.coerce_null_child(nested, expected_type);
            }
            true
        } else {
            false
        }
    }

    pub(crate) fn types_match(&self, expected: &CiprType, actual: &CiprType) -> bool {
        expected == actual
            || *expected == CiprType::Unknown
            || *actual == CiprType::Unknown
            || (*actual == CiprType::Null && matches!(expected, CiprType::Pointer(_)))
            || (*expected == CiprType::Null && matches!(actual, CiprType::Pointer(_)))
    }

    pub(crate) fn reject_opaque_string_construction(
        &mut self,
        struct_name: &str,
        line: usize,
        context: &str,
    ) -> bool {
        if struct_name != "String" {
            return false;
        }

        self.error(
            line,
            &format!(
                "String is opaque; use string_from(...) or a string-producing API instead of {}.",
                context
            ),
        );
        true
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
