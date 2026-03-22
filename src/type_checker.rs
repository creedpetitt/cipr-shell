use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::ast::{CiprType, NodeArena, NodeId, NodeType};
use crate::token::{TokenType, Value};

pub type TypeEnvRef = Rc<RefCell<TypeEnv>>;

pub struct TypeEnv {
    values: HashMap<String, CiprType>,
    enclosing: Option<TypeEnvRef>,
}

impl TypeEnv {
    pub fn new() -> TypeEnvRef {
        Rc::new(RefCell::new(Self {
            values: HashMap::new(),
            enclosing: None,
        }))
    }

    pub fn with_enclosing(enclosing: &TypeEnvRef) -> TypeEnvRef {
        Rc::new(RefCell::new(Self {
            values: HashMap::new(),
            enclosing: Some(Rc::clone(enclosing)),
        }))
    }

    pub fn define(&mut self, name: &str, value_type: CiprType) {
        self.values.insert(name.to_string(), value_type);
    }

    pub fn get(&self, name: &str) -> Option<CiprType> {
        if let Some(val) = self.values.get(name) {
            return Some(val.clone());
        }
        if let Some(ref enc) = self.enclosing {
            return enc.borrow().get(name);
        }
        None
    }
}

pub struct TypeChecker<'a> {
    pub arena: &'a mut NodeArena,
    pub env: TypeEnvRef,
    pub had_error: bool,
    current_return_type: Option<CiprType>,
}

impl<'a> TypeChecker<'a> {
    pub fn new(arena: &'a mut NodeArena) -> Self {
        Self {
            arena,
            env: TypeEnv::new(),
            had_error: false,
            current_return_type: None,
        }
    }

    pub fn define_global(&mut self, name: &str, t: CiprType) {
        self.env.borrow_mut().define(name, t);
    }

    fn error(&mut self, line: usize, message: &str) {
        eprintln!("[line {}] Type Error: {}", line, message);
        self.had_error = true;
    }

    fn parse_type_annotation(annotation: &Option<String>) -> CiprType {
        match annotation {
            Some(s) => match s.as_str() {
                "int" => CiprType::Int,
                "float" => CiprType::Float,
                "str" => CiprType::Str,
                "bool" => CiprType::Bool,
                "void" => CiprType::Void,
                _ => CiprType::Unknown,
            },
            None => CiprType::Unknown,
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
            _ => CiprType::Unknown,
        };

        self.arena[id].resolved_type = t.clone();
        t
    }

    fn check_block(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        for child in children.iter().flatten() {
            self.check(*child);
        }
        CiprType::Void
    }

    fn check_block_stmt(&mut self, id: NodeId) -> CiprType {
        let prev_env = Rc::clone(&self.env);
        self.env = TypeEnv::with_enclosing(&prev_env);

        let children = self.arena[id].children.clone();
        for child in children.iter().flatten() {
            self.check(*child);
        }

        self.env = prev_env;
        CiprType::Void
    }

    fn check_var_decl(&mut self, id: NodeId) -> CiprType {
        let name = self.arena[id].token.lexeme.clone();
        let line = self.arena[id].token.line;
        let annotation = self.arena[id].type_annotation.clone();
        let declared_type = Self::parse_type_annotation(&annotation);

        let children = self.arena[id].children.clone();
        let mut init_type = CiprType::Unknown;
        if let Some(init_id) = children[0] {
            init_type = self.check(init_id);
        }

        let final_type = if declared_type != CiprType::Unknown {
            if init_type != CiprType::Unknown && init_type != declared_type {
                self.error(
                    line,
                    &format!(
                        "Cannot assign {:?} to variable of type {:?}",
                        init_type, declared_type
                    ),
                );
            }
            declared_type
        } else if init_type != CiprType::Unknown {
            init_type
        } else {
            self.error(
                line,
                "Variables must have a type annotation or an initializer.",
            );
            CiprType::Unknown
        };

        self.env.borrow_mut().define(&name, final_type.clone());
        final_type
    }

    fn check_function(&mut self, id: NodeId) -> CiprType {
        let name = self.arena[id].token.lexeme.clone();
        let annotation = self.arena[id].type_annotation.clone();
        let ret_type = Self::parse_type_annotation(&annotation);

        let children = self.arena[id].children.clone();
        let param_count = children.len() - 1;

        let mut param_types = Vec::new();
        for i in 0..param_count {
            if let Some(param_id) = children[i] {
                let p_ann = self.arena[param_id].type_annotation.clone();
                let p_type = Self::parse_type_annotation(&p_ann);
                param_types.push(p_type);
            }
        }

        let func_type = CiprType::Callable(param_types.clone(), Box::new(ret_type.clone()));
        self.env.borrow_mut().define(&name, func_type.clone());

        // Check body
        let prev_env = Rc::clone(&self.env);
        self.env = TypeEnv::with_enclosing(&prev_env);

        for i in 0..param_count {
            if let Some(param_id) = children[i] {
                let p_name = self.arena[param_id].token.lexeme.clone();
                self.env
                    .borrow_mut()
                    .define(&p_name, param_types[i].clone());
                self.arena[param_id].resolved_type = param_types[i].clone();
            }
        }

        let prev_ret = self.current_return_type.clone();
        self.current_return_type = Some(ret_type.clone());

        if let Some(body_id) = children[children.len() - 1] {
            self.check(body_id);
        }

        self.current_return_type = prev_ret;
        self.env = prev_env;

        CiprType::Void
    }

    fn check_expr_stmt(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        if let Some(expr_id) = children[0] {
            self.check(expr_id);
        }
        CiprType::Void
    }

    fn check_if(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        let line = self.arena[id].token.line;
        if let Some(cond_id) = children[0] {
            let cond_type = self.check(cond_id);
            if cond_type != CiprType::Bool && cond_type != CiprType::Unknown {
                self.error(line, "Condition must be a boolean.");
            }
        }
        if let Some(then_id) = children[1] {
            self.check(then_id);
        }
        if let Some(else_id) = children.get(2).copied().flatten() {
            self.check(else_id);
        }
        CiprType::Void
    }

    fn check_while(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        let line = self.arena[id].token.line;
        if let Some(cond_id) = children[0] {
            let cond_type = self.check(cond_id);
            if cond_type != CiprType::Bool && cond_type != CiprType::Unknown {
                self.error(line, "Condition must be a boolean.");
            }
        }
        if let Some(body_id) = children[1] {
            self.check(body_id);
        }
        CiprType::Void
    }

    fn check_return(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        let line = self.arena[id].token.line;

        let mut val_type = CiprType::Void;
        if let Some(val_id) = children[0] {
            val_type = self.check(val_id);
        }

        if let Some(expected) = &self.current_return_type {
            if val_type != *expected && expected != &CiprType::Void && val_type != CiprType::Unknown
            {
                self.error(
                    line,
                    &format!("Expected return type {:?} but got {:?}", expected, val_type),
                );
            }
        } else {
            self.error(line, "Cannot return from top-level code.");
        }

        CiprType::Void
    }

    fn check_literal(&mut self, id: NodeId) -> CiprType {
        match &self.arena[id].value {
            Value::Int(_) => CiprType::Int,
            Value::Float(_) => CiprType::Float,
            Value::Str(_) => CiprType::Str,
            Value::Bool(_) => CiprType::Bool,
            Value::Null => CiprType::Unknown, // Null matches anything for now
        }
    }

    fn check_var_expr(&mut self, id: NodeId) -> CiprType {
        let name = self.arena[id].token.lexeme.clone();
        let line = self.arena[id].token.line;
        let t_opt = self.env.borrow().get(&name);
        if let Some(t) = t_opt {
            t
        } else {
            self.error(line, &format!("Undefined variable '{}'.", name));
            CiprType::Unknown
        }
    }

    fn check_assign(&mut self, id: NodeId) -> CiprType {
        let name = self.arena[id].token.lexeme.clone();
        let line = self.arena[id].token.line;
        let children = self.arena[id].children.clone();

        let val_type = if let Some(val_id) = children[0] {
            self.check(val_id)
        } else {
            CiprType::Unknown
        };

        let var_type_opt = self.env.borrow().get(&name);
        if let Some(var_type) = var_type_opt {
            if var_type != CiprType::Unknown
                && val_type != CiprType::Unknown
                && var_type != val_type
            {
                self.error(
                    line,
                    &format!(
                        "Cannot assign {:?} to variable of type {:?}",
                        val_type, var_type
                    ),
                );
            }
            var_type
        } else {
            self.error(line, &format!("Undefined variable '{}'.", name));
            CiprType::Unknown
        }
    }

    fn check_binary(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        let op = self.arena[id].token.token_type;
        let line = self.arena[id].token.line;

        let left = if let Some(l) = children[0] {
            self.check(l)
        } else {
            CiprType::Unknown
        };
        let right = if let Some(r) = children[1] {
            self.check(r)
        } else {
            CiprType::Unknown
        };

        if left != CiprType::Unknown && right != CiprType::Unknown && left != right {
            self.error(
                line,
                &format!(
                    "Type mismatch in binary operation: {:?} and {:?}",
                    left, right
                ),
            );
        }

        match op {
            TokenType::Plus | TokenType::Minus | TokenType::Star | TokenType::Slash => {
                if left == CiprType::Str && op == TokenType::Plus {
                    CiprType::Str
                } else if left == CiprType::Int {
                    CiprType::Int
                } else if left == CiprType::Float {
                    CiprType::Float
                } else {
                    if left != CiprType::Unknown {
                        self.error(
                            line,
                            &format!("Invalid operands for arithmetic: {:?}", left),
                        );
                    }
                    CiprType::Unknown
                }
            }
            TokenType::Greater
            | TokenType::GreaterEqual
            | TokenType::Less
            | TokenType::LessEqual
            | TokenType::EqualEqual
            | TokenType::BangEqual => CiprType::Bool,
            _ => CiprType::Unknown,
        }
    }

    fn check_unary(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        let op = self.arena[id].token.token_type;
        let line = self.arena[id].token.line;

        let right = if let Some(r) = children[0] {
            self.check(r)
        } else {
            CiprType::Unknown
        };

        match op {
            TokenType::Minus => {
                if right != CiprType::Int && right != CiprType::Float && right != CiprType::Unknown
                {
                    self.error(line, "Operand must be Int or Float.");
                }
                right
            }
            TokenType::Bang => CiprType::Bool,
            _ => CiprType::Unknown,
        }
    }

    fn check_logical(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        if let Some(l) = children[0] {
            self.check(l);
        }
        if let Some(r) = children[1] {
            self.check(r);
        }
        CiprType::Bool
    }

    fn check_call(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        let line = self.arena[id].token.line;

        let callee_type = if let Some(c) = children[0] {
            self.check(c)
        } else {
            CiprType::Unknown
        };

        match callee_type {
            CiprType::Callable(param_types, ret_type) => {
                let arg_count = children.len() - 1;
                if arg_count != param_types.len() {
                    self.error(
                        line,
                        &format!(
                            "Expected {} arguments but got {}",
                            param_types.len(),
                            arg_count
                        ),
                    );
                }
                for i in 0..arg_count {
                    if i < param_types.len() {
                        if let Some(arg_id) = children[i + 1] {
                            let arg_type = self.check(arg_id);
                            if arg_type != CiprType::Unknown
                                && param_types[i] != CiprType::Unknown
                                && arg_type != param_types[i]
                            {
                                self.error(
                                    line,
                                    &format!(
                                        "Expected argument of type {:?} but got {:?}",
                                        param_types[i], arg_type
                                    ),
                                );
                            }
                        }
                    }
                }
                *ret_type
            }
            CiprType::Unknown => CiprType::Unknown,
            _ => {
                self.error(line, "Can only call functions.");
                CiprType::Unknown
            }
        }
    }

    fn check_array(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        let mut elem_type = CiprType::Unknown;

        for c in children.iter().flatten() {
            let t = self.check(*c);
            if elem_type == CiprType::Unknown {
                elem_type = t;
            } else if t != CiprType::Unknown && t != elem_type {
                self.error(
                    self.arena[id].token.line,
                    "Array elements must have the same type.",
                );
            }
        }

        CiprType::Array(Box::new(elem_type))
    }

    fn check_index_get(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        let line = self.arena[id].token.line;

        let target_type = if let Some(c) = children[0] {
            self.check(c)
        } else {
            CiprType::Unknown
        };
        let index_type = if let Some(c) = children[1] {
            self.check(c)
        } else {
            CiprType::Unknown
        };

        if index_type != CiprType::Int && index_type != CiprType::Unknown {
            self.error(line, "Array index must be an Int.");
        }

        match target_type {
            CiprType::Array(inner) => *inner,
            CiprType::Unknown => CiprType::Unknown,
            _ => {
                self.error(line, "Only arrays can be indexed.");
                CiprType::Unknown
            }
        }
    }
}
