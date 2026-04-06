use crate::ast::{alloc_node, CiprType, NodeId, NodeType};
use crate::token::{Token, TokenType, Value};
use crate::type_checker::TypeChecker;

impl<'a> TypeChecker<'a> {
    fn maybe_rewrite_ufcs_call(&mut self, call_id: NodeId) -> Option<CiprType> {
        let call_children = self.arena[call_id].children.clone();
        let Some(callee_id) = call_children[0] else {
            return Some(CiprType::Unknown);
        };

        if self.arena[callee_id].node_type != NodeType::GetField {
            return None;
        }

        let get_children = self.arena[callee_id].children.clone();
        let Some(receiver_id) = get_children[0] else {
            return Some(CiprType::Unknown);
        };

        let receiver_type = self.check(receiver_id);
        let method_name = self.arena[callee_id].token.lexeme.clone();
        let line = self.arena[call_id].token.line;

        // Real fields keep field-call behavior (including non-callable errors).
        if self.receiver_has_field(&receiver_type, &method_name) {
            return None;
        }

        let mut candidates = Vec::new();
        if let Some(base_type_name) = Self::ufcs_base_type_name(&receiver_type) {
            candidates.push(format!("{}_{}", base_type_name, method_name));
        }
        candidates.push(method_name.clone());

        for candidate in &candidates {
            let Some(candidate_type) = self.env.get(candidate) else {
                continue;
            };
            if !self.ufcs_receiver_compatible(&candidate_type, &receiver_type) {
                continue;
            }

            let callee_token = Token::synthetic(TokenType::Identifier, candidate, line);
            let rewritten_callee = alloc_node(
                self.arena,
                NodeType::VarExpr,
                callee_token,
                Value::Null,
                vec![],
            );

            let mut rewritten_children = vec![Some(rewritten_callee), Some(receiver_id)];
            rewritten_children.extend(call_children.iter().skip(1).copied());
            self.arena[call_id].children = rewritten_children;
            return None;
        }

        self.error(
            line,
            &format!(
                "No method '{}' for receiver type {:?}. Tried: {}.",
                method_name,
                receiver_type,
                candidates.join(", ")
            ),
        );
        Some(CiprType::Unknown)
    }

    fn receiver_has_field(&self, receiver_type: &CiprType, field_name: &str) -> bool {
        let struct_name_opt = match receiver_type {
            CiprType::Struct(name) => Some(name),
            CiprType::Pointer(inner) => match inner.as_ref() {
                CiprType::Struct(name) => Some(name),
                _ => None,
            },
            _ => None,
        };

        let Some(struct_name) = struct_name_opt else {
            return false;
        };
        let Some(fields) = self.structs.get(struct_name) else {
            return false;
        };
        fields.iter().any(|(name, _)| name == field_name)
    }

    fn ufcs_base_type_name(receiver_type: &CiprType) -> Option<String> {
        match receiver_type {
            CiprType::Struct(name) => Some(name.clone()),
            CiprType::Pointer(inner) => match inner.as_ref() {
                CiprType::Struct(name) => Some(name.clone()),
                _ => None,
            },
            CiprType::Str => Some("str".to_string()),
            _ => None,
        }
    }

    fn ufcs_receiver_compatible(&self, candidate_type: &CiprType, receiver_type: &CiprType) -> bool {
        let CiprType::Callable(params, _) = candidate_type else {
            return false;
        };
        let Some(first_param) = params.first() else {
            return false;
        };
        self.types_match(first_param, receiver_type)
    }

    pub(crate) fn check_call(&mut self, id: NodeId) -> CiprType {
        if let Some(t) = self.maybe_rewrite_ufcs_call(id) {
            return t;
        }

        let children = self.arena[id].children.clone();
        let line = self.arena[id].token.line;

        let callee_type = self.check_child(children[0]);

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
                        let arg_type = self.check_child(children[i + 1]);
                        if arg_type == CiprType::Null && param_types[i] == CiprType::Unknown {
                            self.error(
                                line,
                                "Null arguments require a parameter with an explicit pointer type.",
                            );
                        } else if arg_type == CiprType::Null
                            && !self.coerce_null_child(children[i + 1], &param_types[i])
                        {
                            self.error(line, "Null can only be passed to pointer parameters.");
                        } else if !self.types_match(&param_types[i], &arg_type)
                            && arg_type != CiprType::Unknown
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
                *ret_type
            }
            CiprType::Unknown => CiprType::Unknown,
            _ => {
                self.error(line, "Can only call functions.");
                CiprType::Unknown
            }
        }
    }

    pub(crate) fn check_extern_fn(&mut self, id: NodeId) -> CiprType {
        let name = self.arena[id].token.lexeme.clone();
        let children = self.arena[id].children.clone();
        let ret_ann = self.arena[id].type_annotation.clone();
        let ret_type = Self::parse_type_annotation(&ret_ann);
        self.validate_return_type(
            &ret_type,
            self.arena[id].token.line,
            "Extern function return type",
        );

        self.arena[id].resolved_type = ret_type.clone();

        let mut param_types = Vec::new();
        for child_opt in children {
            if let Some(param_id) = child_opt {
                let p_ann = self.arena[param_id].type_annotation.clone();
                let p_type = Self::parse_type_annotation(&p_ann);
                self.validate_value_type(
                    &p_type,
                    self.arena[param_id].token.line,
                    "Extern function parameter type",
                );
                self.arena[param_id].resolved_type = p_type.clone();
                param_types.push(p_type);
            }
        }

        let func_type = CiprType::Callable(param_types, Box::new(ret_type));
        self.env.define(&name, func_type.clone());
        func_type
    }

    pub(crate) fn check_function(&mut self, id: NodeId) -> CiprType {
        let name = self.arena[id].token.lexeme.clone();
        let annotation = self.arena[id].type_annotation.clone();
        let ret_type = Self::parse_type_annotation(&annotation);
        self.validate_return_type(&ret_type, self.arena[id].token.line, "Function return type");

        let children = self.arena[id].children.clone();
        let param_count = children.len() - 1;

        let mut param_types = Vec::new();
        for i in 0..param_count {
            if let Some(param_id) = children[i] {
                let p_ann = self.arena[param_id].type_annotation.clone();
                let p_type = Self::parse_type_annotation(&p_ann);
                self.validate_value_type(
                    &p_type,
                    self.arena[param_id].token.line,
                    "Function parameter type",
                );
                param_types.push(p_type);
            }
        }

        let func_type = CiprType::Callable(param_types.clone(), Box::new(ret_type.clone()));
        self.env.define(&name, func_type.clone());

        // Check body
        self.env.enter_scope();

        for i in 0..param_count {
            if let Some(param_id) = children[i] {
                let p_name = self.arena[param_id].token.lexeme.clone();
                self.env.define(&p_name, param_types[i].clone());
                self.arena[param_id].resolved_type = param_types[i].clone();
            }
        }

        let prev_ret = self.current_return_type.clone();
        self.current_return_type = Some(ret_type.clone());

        if let Some(body_id) = children[children.len() - 1] {
            self.check(body_id);
        }

        self.current_return_type = prev_ret;
        self.env.exit_scope();

        func_type
    }

    pub(crate) fn check_include(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        self.predeclare_structs(&children);
        for child in children.iter().flatten() {
            self.check(*child);
        }
        CiprType::Void
    }
}
