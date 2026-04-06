use crate::ast::{alloc_node, alloc_node_typed, CiprType, NodeArena, NodeId, NodeType};
use crate::diagnostics::{DiagnosticPhase, Diagnostics};
use crate::token::{Token, TokenType, Value};
use crate::type_syntax;
use std::collections::HashSet;

pub struct Parser<'a> {
    tokens: &'a [Token],
    pub arena: &'a mut NodeArena,
    current: usize,
    pub had_error: bool,
    visited_files: &'a mut HashSet<String>,
    source_name: String,
    diagnostics: Diagnostics,
}

impl<'a> Parser<'a> {
    pub fn new_with_source(
        tokens: &'a [Token],
        arena: &'a mut NodeArena,
        visited_files: &'a mut HashSet<String>,
        source_name: &str,
    ) -> Self {
        Self {
            tokens,
            arena,
            current: 0,
            had_error: false,
            visited_files,
            source_name: source_name.to_string(),
            diagnostics: Diagnostics::new(),
        }
    }

    pub fn parse(&mut self) -> Option<NodeId> {
        let mut statements: Vec<Option<NodeId>> = Vec::new();

        while !self.is_at_end() {
            statements.push(self.declaration());
        }

        let prev = if self.current > 0 {
            self.previous().clone()
        } else {
            self.peek().clone()
        };
        Some(alloc_node(
            self.arena,
            NodeType::StmtList,
            prev,
            Value::Null,
            statements,
        ))
    }

    pub fn take_diagnostics(&mut self) -> Diagnostics {
        std::mem::take(&mut self.diagnostics)
    }

    // ── Declarations ──

    fn declaration(&mut self) -> Option<NodeId> {
        let result = if self.match_types(&[TokenType::Struct]) {
            self.struct_declaration()
        } else if self.match_types(&[TokenType::Fn]) {
            self.function("function")
        } else if self.match_types(&[TokenType::Let]) {
            self.var_declaration()
        } else if self.match_types(&[TokenType::Extern]) {
            self.extern_declaration()
        } else if self.match_types(&[TokenType::Include]) {
            self.include_declaration()
        } else {
            self.statement()
        };

        match result {
            Some(id) => Some(id),
            None => {
                self.synchronize();
                None
            }
        }
    }

    fn var_declaration(&mut self) -> Option<NodeId> {
        let name = self.consume(TokenType::Identifier, "Expect variable name.")?;

        let mut type_annotation = None;
        if self.match_types(&[TokenType::Colon]) {
            type_annotation = self.parse_type_annotation();
        }

        let initializer = if self.match_types(&[TokenType::Equal]) {
            self.expression()
        } else {
            None
        };

        self.consume(TokenType::Semicolon, "Expected ';' after declaration")?;

        Some(alloc_node_typed(
            self.arena,
            NodeType::StmtVarDecl,
            name,
            Value::Null,
            vec![initializer],
            type_annotation,
        ))
    }

    fn struct_declaration(&mut self) -> Option<NodeId> {
        let name = self.consume(TokenType::Identifier, "Expect struct name.")?;
        self.consume(TokenType::LeftBrace, "Expect '{' before struct body.")?;

        let mut fields: Vec<Option<NodeId>> = Vec::new();
        while !self.check(TokenType::RightBrace) && !self.is_at_end() {
            let field_name = self.consume(TokenType::Identifier, "Expect field name.")?;
            self.consume(TokenType::Colon, "Expect ':' after field name.")?;
            let field_type = self.parse_type_annotation();

            let field_node = alloc_node_typed(
                self.arena,
                NodeType::VarExpr,
                field_name,
                Value::Null,
                vec![],
                field_type,
            );
            fields.push(Some(field_node));

            if !self.match_types(&[TokenType::Comma]) {
                break;
            }
        }
        self.consume(TokenType::RightBrace, "Expect '}' after struct body.")?;

        Some(alloc_node(
            self.arena,
            NodeType::StmtStructDecl,
            name,
            Value::Null,
            fields,
        ))
    }

    fn function(&mut self, kind: &str) -> Option<NodeId> {
        let name = self.consume(TokenType::Identifier, &format!("Expect {kind} name."))?;
        self.consume(
            TokenType::LeftParen,
            &format!("Expect '(' after {kind} name."),
        )?;

        let mut parameters: Vec<Option<NodeId>> = Vec::new();
        if !self.check(TokenType::RightParen) {
            loop {
                if parameters.len() >= 255 {
                    self.error_at_peek("Can't have more than 255 parameters.");
                }
                let param_name = self.consume(TokenType::Identifier, "Expect parameter name.")?;

                let mut type_annotation = None;
                if self.match_types(&[TokenType::Colon]) {
                    type_annotation = self.parse_type_annotation();
                }

                let param_node = alloc_node_typed(
                    self.arena,
                    NodeType::VarExpr,
                    param_name,
                    Value::Null,
                    vec![],
                    type_annotation,
                );
                parameters.push(Some(param_node));
                if !self.match_types(&[TokenType::Comma]) {
                    break;
                }
            }
        }
        self.consume(TokenType::RightParen, "Expect ')' after parameters.")?;

        let mut return_type = None;
        if self.match_types(&[TokenType::Colon]) {
            return_type = self.parse_type_annotation();
        }

        self.consume(
            TokenType::LeftBrace,
            &format!("Expect '{{' before {kind} body."),
        )?;
        let body_stmts = self.block()?;
        let prev = self.previous().clone();
        let body_node = alloc_node(
            self.arena,
            NodeType::StmtBlock,
            prev,
            Value::Null,
            body_stmts,
        );

        let mut children = parameters;
        children.push(Some(body_node));

        Some(alloc_node_typed(
            self.arena,
            NodeType::StmtFunction,
            name,
            Value::Null,
            children,
            return_type,
        ))
    }

    fn extern_declaration(&mut self) -> Option<NodeId> {
        self.consume(TokenType::Fn, "Expect 'fn' after 'extern'.")?;
        let name = self.consume(TokenType::Identifier, "Expect extern function name.")?;
        self.consume(
            TokenType::LeftParen,
            "Expect '(' after extern function name.",
        )?;

        let mut parameters: Vec<Option<NodeId>> = Vec::new();
        if !self.check(TokenType::RightParen) {
            loop {
                let param_name = self.consume(TokenType::Identifier, "Expect parameter name.")?;
                let mut type_annotation = None;
                if self.match_types(&[TokenType::Colon]) {
                    type_annotation = self.parse_type_annotation();
                }

                let param_node = alloc_node_typed(
                    self.arena,
                    NodeType::VarExpr,
                    param_name,
                    Value::Null,
                    vec![],
                    type_annotation,
                );
                parameters.push(Some(param_node));
                if !self.match_types(&[TokenType::Comma]) {
                    break;
                }
            }
        }
        self.consume(TokenType::RightParen, "Expect ')' after parameters.")?;

        let mut return_type = None;
        if self.match_types(&[TokenType::Colon]) {
            return_type = self.parse_type_annotation();
        }

        self.consume(TokenType::Semicolon, "Expect ';' after extern declaration.")?;

        Some(alloc_node_typed(
            self.arena,
            NodeType::StmtExternFn,
            name,
            Value::Null,
            parameters,
            return_type,
        ))
    }

    fn include_declaration(&mut self) -> Option<NodeId> {
        let path = self.consume(TokenType::Str, "Expect string path after 'include'.")?;
        self.consume(TokenType::Semicolon, "Expect ';' after include statement.")?;

        let path_str = match &path.literal {
            Value::Str(s) => s.clone(),
            _ => return None,
        };

        let mut children = Vec::new();

        if self.visited_files.insert(path_str.clone()) {
            match std::fs::read_to_string(&path_str) {
                Ok(source) => {
                    let (tokens, scan_error, scan_diags) =
                        crate::scanner::Scanner::new_with_source(&source, &path_str)
                            .scan_tokens_with_diagnostics();
                    self.diagnostics.extend(scan_diags);
                    if scan_error {
                        self.error_at(&path, "Included file has scanner errors.");
                    } else {
                        let (root_id, sub_had_error) = {
                            let mut sub_parser = Parser::new_with_source(
                                &tokens,
                                &mut *self.arena,
                                &mut *self.visited_files,
                                &path_str,
                            );
                            let root_id = sub_parser.parse();
                            let sub_had_error = sub_parser.had_error;
                            let sub_diags = sub_parser.take_diagnostics();
                            self.diagnostics.extend(sub_diags);
                            (root_id, sub_had_error)
                        };

                        if sub_had_error {
                            self.error_at(&path, "Included file has parser errors.");
                        }

                        if let Some(id) = root_id {
                            children = self.arena[id].children.clone();
                        }
                    }
                }
                Err(_) => {
                    self.error_at(
                        &path,
                        &format!("Could not read included file '{}'.", path_str),
                    );
                }
            }
        }

        Some(alloc_node(
            self.arena,
            NodeType::StmtInclude,
            path.clone(),
            Value::Null,
            children,
        ))
    }

    // ── Statements ──

    fn statement(&mut self) -> Option<NodeId> {
        if self.match_types(&[TokenType::If]) {
            return self.if_statement();
        }
        if self.match_types(&[TokenType::While]) {
            return self.while_statement();
        }
        if self.match_types(&[TokenType::For]) {
            return self.for_statement();
        }
        if self.match_types(&[TokenType::Return]) {
            return self.return_statement();
        }
        if self.match_types(&[TokenType::Delete]) {
            return self.delete_statement();
        }
        if self.match_types(&[TokenType::LeftBrace]) {
            let statements = self.block()?;
            let prev = self.previous().clone();
            return Some(alloc_node(
                self.arena,
                NodeType::StmtBlock,
                prev,
                Value::Null,
                statements,
            ));
        }

        self.expression_statement()
    }

    fn expression_statement(&mut self) -> Option<NodeId> {
        let expr = self.expression();
        self.consume(TokenType::Semicolon, "Expected ';' after value")?;
        let prev = self.previous().clone();
        Some(alloc_node(
            self.arena,
            NodeType::StmtExpr,
            prev,
            Value::Null,
            vec![expr],
        ))
    }

    fn delete_statement(&mut self) -> Option<NodeId> {
        let keyword = self.previous().clone();
        let value = self.expression()?;
        self.consume(
            TokenType::Semicolon,
            "Expected ';' after expression to delete.",
        )?;
        Some(alloc_node(
            self.arena,
            NodeType::StmtDelete,
            keyword,
            Value::Null,
            vec![Some(value)],
        ))
    }

    fn if_statement(&mut self) -> Option<NodeId> {
        let condition = self.consume_condition("if")?;
        let then_branch = self.statement();

        let else_branch = if self.match_types(&[TokenType::Else]) {
            self.statement()
        } else {
            None
        };

        let prev = self.previous().clone();
        Some(alloc_node(
            self.arena,
            NodeType::StmtIf,
            prev,
            Value::Null,
            vec![Some(condition), then_branch, else_branch],
        ))
    }

    fn while_statement(&mut self) -> Option<NodeId> {
        let condition = self.consume_condition("while")?;
        let body = self.statement();
        let prev = self.previous().clone();
        Some(alloc_node(
            self.arena,
            NodeType::StmtWhile,
            prev,
            Value::Null,
            vec![Some(condition), body],
        ))
    }

    fn for_statement(&mut self) -> Option<NodeId> {
        self.consume(TokenType::LeftParen, "Expect '(' after 'for'.")?;

        let initializer = if self.match_types(&[TokenType::Semicolon]) {
            None
        } else if self.match_types(&[TokenType::Let]) {
            self.var_declaration()
        } else {
            self.expression_statement()
        };

        let condition_expr = if !self.check(TokenType::Semicolon) {
            self.expression()
        } else {
            None
        };
        self.consume(TokenType::Semicolon, "Expect ';' after loop condition")?;

        let increment = if !self.check(TokenType::RightParen) {
            self.expression()
        } else {
            None
        };
        self.consume(TokenType::RightParen, "Expect ')' after for clauses.")?;

        let mut body = self.statement();

        // Desugar increment into body
        if let Some(incr) = increment {
            let prev = self.previous().clone();
            let incr_stmt = alloc_node(
                self.arena,
                NodeType::StmtExpr,
                prev.clone(),
                Value::Null,
                vec![Some(incr)],
            );
            body = Some(alloc_node(
                self.arena,
                NodeType::StmtBlock,
                prev,
                Value::Null,
                vec![body, Some(incr_stmt)],
            ));
        }

        // Desugar condition (default to true)
        let condition = match condition_expr {
            Some(c) => Some(c),
            None => {
                let true_tok =
                    Token::new(TokenType::True, "true".to_string(), Value::Bool(true), 0);
                Some(alloc_node(
                    self.arena,
                    NodeType::Literal,
                    true_tok,
                    Value::Bool(true),
                    vec![],
                ))
            }
        };

        let prev = self.previous().clone();
        body = Some(alloc_node(
            self.arena,
            NodeType::StmtWhile,
            prev.clone(),
            Value::Null,
            vec![condition, body],
        ));

        if let Some(init) = initializer {
            body = Some(alloc_node(
                self.arena,
                NodeType::StmtBlock,
                prev,
                Value::Null,
                vec![Some(init), body],
            ));
        }

        body
    }

    fn return_statement(&mut self) -> Option<NodeId> {
        let keyword = self.previous().clone();
        let value = if !self.check(TokenType::Semicolon) {
            self.expression()
        } else {
            None
        };
        self.consume(TokenType::Semicolon, "Expect ';' after return value.")?;
        Some(alloc_node(
            self.arena,
            NodeType::StmtReturn,
            keyword,
            Value::Null,
            vec![value],
        ))
    }

    fn block(&mut self) -> Option<Vec<Option<NodeId>>> {
        let mut statements: Vec<Option<NodeId>> = Vec::new();

        while !self.check(TokenType::RightBrace) && !self.is_at_end() {
            statements.push(self.declaration());
        }

        self.consume(TokenType::RightBrace, "Expect '}' after block.")?;
        Some(statements)
    }

    fn consume_condition(&mut self, name: &str) -> Option<NodeId> {
        self.consume(
            TokenType::LeftParen,
            &format!("Expect '(' after '{name}' statement."),
        )?;
        let condition = self.expression()?;
        self.consume(
            TokenType::RightParen,
            &format!("Expect ')' after {name} condition."),
        )?;
        Some(condition)
    }

    // ── Expressions ──

    fn expression(&mut self) -> Option<NodeId> {
        self.assignment()
    }

    fn assignment(&mut self) -> Option<NodeId> {
        let expr = self.logical_or()?;

        if self.match_types(&[TokenType::Equal]) {
            let equals = self.previous().clone();
            let value = self.assignment();

            let left_type = self.arena[expr].node_type;
            if left_type == NodeType::VarExpr {
                let name = self.arena[expr].token.clone();
                return Some(alloc_node(
                    self.arena,
                    NodeType::Assign,
                    name,
                    Value::Null,
                    vec![value],
                ));
            } else if left_type == NodeType::GetField {
                let name = self.arena[expr].token.clone();
                let target_expr = self.arena[expr].children[0];
                return Some(alloc_node(
                    self.arena,
                    NodeType::AssignField,
                    name,
                    Value::Null,
                    vec![target_expr, value],
                ));
            } else if left_type == NodeType::Dereference {
                let equals = self.previous().clone();
                let inner_expr = self.arena[expr].children[0];
                return Some(alloc_node(
                    self.arena,
                    NodeType::AssignDeref,
                    equals,
                    Value::Null,
                    vec![inner_expr, value],
                ));
            } else if left_type == NodeType::IndexGet {
                let address_of = alloc_node(
                    self.arena,
                    NodeType::AddressOf,
                    Token::synthetic(TokenType::At, "@", equals.line),
                    Value::Null,
                    vec![Some(expr)],
                );
                return Some(alloc_node(
                    self.arena,
                    NodeType::AssignDeref,
                    equals,
                    Value::Null,
                    vec![Some(address_of), value],
                ));
            }

            self.error_at(&equals, "Invalid assignment target.");
        }

        Some(expr)
    }

    fn logical_or(&mut self) -> Option<NodeId> {
        let mut left = self.logical_and()?;

        while self.match_types(&[TokenType::Or]) {
            let op = self.previous().clone();
            let right = self.logical_and()?;
            left = alloc_node(
                self.arena,
                NodeType::Logical,
                op,
                Value::Null,
                vec![Some(left), Some(right)],
            );
        }

        Some(left)
    }

    fn logical_and(&mut self) -> Option<NodeId> {
        let mut left = self.equality()?;

        while self.match_types(&[TokenType::And]) {
            let op = self.previous().clone();
            let right = self.equality()?;
            left = alloc_node(
                self.arena,
                NodeType::Logical,
                op,
                Value::Null,
                vec![Some(left), Some(right)],
            );
        }

        Some(left)
    }

    fn equality(&mut self) -> Option<NodeId> {
        let mut left = self.comparison()?;

        while self.match_types(&[TokenType::BangEqual, TokenType::EqualEqual]) {
            let op = self.previous().clone();
            let right = self.comparison()?;
            left = alloc_node(
                self.arena,
                NodeType::Binary,
                op,
                Value::Null,
                vec![Some(left), Some(right)],
            );
        }

        Some(left)
    }

    fn comparison(&mut self) -> Option<NodeId> {
        let mut left = self.term()?;

        while self.match_types(&[
            TokenType::Greater,
            TokenType::GreaterEqual,
            TokenType::Less,
            TokenType::LessEqual,
        ]) {
            let op = self.previous().clone();
            let right = self.term()?;
            left = alloc_node(
                self.arena,
                NodeType::Binary,
                op,
                Value::Null,
                vec![Some(left), Some(right)],
            );
        }

        Some(left)
    }

    fn term(&mut self) -> Option<NodeId> {
        let mut left = self.factor()?;

        while self.match_types(&[TokenType::Plus, TokenType::Minus]) {
            let op = self.previous().clone();
            let right = self.factor()?;
            left = alloc_node(
                self.arena,
                NodeType::Binary,
                op,
                Value::Null,
                vec![Some(left), Some(right)],
            );
        }

        Some(left)
    }

    fn factor(&mut self) -> Option<NodeId> {
        let mut left = self.unary()?;

        while self.match_types(&[TokenType::Slash, TokenType::Star]) {
            let op = self.previous().clone();
            let right = self.unary()?;
            left = alloc_node(
                self.arena,
                NodeType::Binary,
                op,
                Value::Null,
                vec![Some(left), Some(right)],
            );
        }

        Some(left)
    }

    fn unary(&mut self) -> Option<NodeId> {
        if self.match_types(&[TokenType::Bang, TokenType::Minus, TokenType::At]) {
            let op = self.previous().clone();
            let right = self.unary();
            let node_type = if op.token_type == TokenType::At {
                NodeType::AddressOf
            } else {
                NodeType::Unary
            };
            return Some(alloc_node(
                self.arena,
                node_type,
                op,
                Value::Null,
                vec![right],
            ));
        }

        self.call()
    }

    fn call(&mut self) -> Option<NodeId> {
        let mut expr = self.primary()?;

        loop {
            if self.match_types(&[TokenType::LeftParen]) {
                expr = self.finish_call(expr)?;
            } else if self.match_types(&[TokenType::LeftBracket]) {
                expr = self.finish_index(expr)?;
            } else if self.match_types(&[TokenType::Dot]) {
                let name =
                    self.consume(TokenType::Identifier, "Expect property name after '.'.")?;
                expr = alloc_node(
                    self.arena,
                    NodeType::GetField,
                    name,
                    Value::Null,
                    vec![Some(expr)],
                );
            } else if self.match_types(&[TokenType::At]) {
                let op = self.previous().clone();
                expr = alloc_node(
                    self.arena,
                    NodeType::Dereference,
                    op,
                    Value::Null,
                    vec![Some(expr)],
                );
            } else {
                break;
            }
        }

        Some(expr)
    }

    fn finish_call(&mut self, callee: NodeId) -> Option<NodeId> {
        let mut arguments: Vec<Option<NodeId>> = vec![Some(callee)];

        if !self.check(TokenType::RightParen) {
            loop {
                if arguments.len() > 255 {
                    self.error_at_peek("Can't have more than 255 arguments.");
                }
                arguments.push(self.expression());
                if !self.match_types(&[TokenType::Comma]) {
                    break;
                }
            }
        }

        let paren = self.consume(TokenType::RightParen, "Expect ')' after arguments.")?;
        Some(alloc_node(
            self.arena,
            NodeType::Call,
            paren,
            Value::Null,
            arguments,
        ))
    }

    fn finish_index(&mut self, callee: NodeId) -> Option<NodeId> {
        let index = self.expression();
        let bracket = self.consume(TokenType::RightBracket, "Expect ']' after index.")?;
        Some(alloc_node(
            self.arena,
            NodeType::IndexGet,
            bracket,
            Value::Null,
            vec![Some(callee), index],
        ))
    }

    fn array(&mut self) -> Option<NodeId> {
        let bracket = self.previous().clone();
        let mut elements: Vec<Option<NodeId>> = Vec::new();

        if !self.check(TokenType::RightBracket) {
            loop {
                elements.push(self.expression());
                if !self.match_types(&[TokenType::Comma]) {
                    break;
                }
            }
        }
        self.consume(TokenType::RightBracket, "Expect ']' after array elements.")?;
        Some(alloc_node(
            self.arena,
            NodeType::Array,
            bracket,
            Value::Null,
            elements,
        ))
    }

    fn primary(&mut self) -> Option<NodeId> {
        if self.match_types(&[TokenType::New]) {
            let struct_name =
                self.consume(TokenType::Identifier, "Expect struct name after 'new'.")?;
            self.consume(TokenType::LeftParen, "Expect '(' after struct name.")?;

            let mut args = Vec::new();
            if !self.check(TokenType::RightParen) {
                loop {
                    args.push(Some(self.expression()?));
                    if !self.match_types(&[TokenType::Comma]) {
                        break;
                    }
                }
            }
            self.consume(TokenType::RightParen, "Expect ')' after arguments.")?;

            if let Some(desugared) = self.try_desugar_builtin_new(&struct_name, &args) {
                return Some(desugared);
            }

            return Some(alloc_node(
                self.arena,
                NodeType::ExprNew,
                struct_name,
                Value::Null,
                args,
            ));
        }

        if self.match_types(&[TokenType::False]) {
            let prev = self.previous().clone();
            return Some(alloc_node(
                self.arena,
                NodeType::Literal,
                prev,
                Value::Bool(false),
                vec![],
            ));
        }
        if self.match_types(&[TokenType::True]) {
            let prev = self.previous().clone();
            return Some(alloc_node(
                self.arena,
                NodeType::Literal,
                prev,
                Value::Bool(true),
                vec![],
            ));
        }
        if self.match_types(&[TokenType::Null]) {
            let prev = self.previous().clone();
            return Some(alloc_node(
                self.arena,
                NodeType::Literal,
                prev,
                Value::Null,
                vec![],
            ));
        }

        if self.match_types(&[TokenType::Int, TokenType::Float, TokenType::Str]) {
            let prev = self.previous().clone();
            let val = prev.literal.clone();
            return Some(alloc_node(self.arena, NodeType::Literal, prev, val, vec![]));
        }

        if self.match_types(&[TokenType::LeftBracket]) {
            return self.array();
        }

        if self.match_types(&[TokenType::LeftParen]) {
            let expr = self.expression();
            self.consume(TokenType::RightParen, "Expect ')' after expression.")?;
            let prev = self.previous().clone();
            return Some(alloc_node(
                self.arena,
                NodeType::Grouping,
                prev,
                Value::Null,
                vec![expr],
            ));
        }

        if self.match_types(&[TokenType::Identifier]) {
            let prev = self.previous().clone();

            if self.match_types(&[TokenType::LeftBrace]) {
                let mut field_inits = Vec::new();
                while !self.check(TokenType::RightBrace) && !self.is_at_end() {
                    let field_name = self.consume(TokenType::Identifier, "Expect field name.")?;
                    self.consume(TokenType::Colon, "Expect ':' after field name.")?;
                    let expr = self.expression();

                    let field_node = alloc_node(
                        self.arena,
                        NodeType::Assign,
                        field_name,
                        Value::Null,
                        vec![expr],
                    );
                    field_inits.push(Some(field_node));

                    if !self.match_types(&[TokenType::Comma]) {
                        break;
                    }
                }
                self.consume(
                    TokenType::RightBrace,
                    "Expect '}' after struct initialization.",
                )?;
                return Some(alloc_node(
                    self.arena,
                    NodeType::StructInit,
                    prev,
                    Value::Null,
                    field_inits,
                ));
            }

            return Some(alloc_node(
                self.arena,
                NodeType::VarExpr,
                prev,
                Value::Null,
                vec![],
            ));
        }

        self.error_at_peek("Expect expression.");
        None
    }

    fn parse_type_annotation(&mut self) -> Option<CiprType> {
        let mut pointer_depth = 0usize;
        while self.match_types(&[TokenType::At]) {
            pointer_depth += 1;
        }

        let mut core = if self.match_types(&[TokenType::Fn]) {
            self.consume(
                TokenType::LeftParen,
                "Expect '(' after 'fn' in function type.",
            )?;

            let mut params = Vec::new();
            if !self.check(TokenType::RightParen) {
                loop {
                    params.push(self.parse_type_annotation()?);
                    if !self.match_types(&[TokenType::Comma]) {
                        break;
                    }
                }
            }

            self.consume(
                TokenType::RightParen,
                "Expect ')' after function type parameters.",
            )?;
            self.consume(TokenType::Colon, "Expect ':' before function type return.")?;
            let ret = self.parse_type_annotation()?;
            CiprType::Callable(params, Box::new(ret))
        } else {
            let type_token = self.consume(TokenType::Identifier, "Expect type name.")?;
            type_syntax::parse_named_type(&type_token.lexeme)
        };

        for _ in 0..pointer_depth {
            core = CiprType::Pointer(Box::new(core));
        }

        Some(core)
    }

    // ── Utilities ──

    fn try_desugar_builtin_new(
        &mut self,
        struct_name: &Token,
        args: &[Option<NodeId>],
    ) -> Option<NodeId> {
        // Reserved built-in heap constructors lowered to stdlib function calls.
        let (mapped_fn, expected_arity) = match struct_name.lexeme.as_str() {
            "String" => ("string_from", 1usize),
            "IntVec" => ("int_vec_new", 0usize),
            "StrVec" => ("str_vec_new", 0usize),
            "StrIntMap" => ("str_int_map_new", 0usize),
            "StrStrMap" => ("str_str_map_new", 0usize),
            _ => return None,
        };

        if args.len() != expected_arity {
            self.error_at(
                struct_name,
                &format!(
                    "'new {}' expects {} arguments but got {}.",
                    struct_name.lexeme,
                    expected_arity,
                    args.len()
                ),
            );

            return Some(alloc_node(
                self.arena,
                NodeType::Literal,
                Token::synthetic(TokenType::Null, "null", struct_name.line),
                Value::Null,
                vec![],
            ));
        }

        let fn_tok = Token::synthetic(TokenType::Identifier, mapped_fn, struct_name.line);
        let callee = alloc_node(self.arena, NodeType::VarExpr, fn_tok, Value::Null, vec![]);

        let mut call_args = vec![Some(callee)];
        call_args.extend(args.iter().copied());

        Some(alloc_node(
            self.arena,
            NodeType::Call,
            struct_name.clone(),
            Value::Null,
            call_args,
        ))
    }

    fn match_types(&mut self, types: &[TokenType]) -> bool {
        for &tt in types {
            if self.check(tt) {
                self.advance();
                return true;
            }
        }
        false
    }

    fn check(&self, tt: TokenType) -> bool {
        if self.is_at_end() {
            return false;
        }
        self.peek().token_type == tt
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.previous()
    }

    fn is_at_end(&self) -> bool {
        self.peek().token_type == TokenType::EofToken
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.current]
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.current - 1]
    }

    fn consume(&mut self, tt: TokenType, message: &str) -> Option<Token> {
        if self.check(tt) {
            self.advance();
            return Some(self.previous().clone());
        }
        self.error_at_peek(message);
        None
    }

    fn error_at_peek(&mut self, message: &str) {
        let token = self.peek().clone();
        self.error_at(&token, message);
    }

    fn error_at(&mut self, token: &Token, message: &str) {
        let detail = format!("Error at '{}': {}", token.lexeme, message);
        self.diagnostics.emit_line(
            DiagnosticPhase::Parse,
            &self.source_name,
            token.line,
            &detail,
        );
        self.had_error = true;
    }

    fn synchronize(&mut self) {
        self.advance();

        while !self.is_at_end() {
            if self.previous().token_type == TokenType::Semicolon {
                return;
            }

            match self.peek().token_type {
                TokenType::Fn
                | TokenType::Let
                | TokenType::For
                | TokenType::If
                | TokenType::While
                | TokenType::Return => return,
                _ => {}
            }

            self.advance();
        }
    }
}
