use crate::ast::NodeId;
use crate::codegen::Codegen;
use crate::token::TokenType;
use inkwell::values::BasicValueEnum;

impl<'a, 'ctx> Codegen<'a, 'ctx> {
    pub(crate) fn visit_if(&mut self, id: NodeId) -> Result<(), String> {
        let children = self.arena[id].children.clone();

        let cond_id = children[0].expect("If missing condition");
        let cond_val = self.evaluate(cond_id)?.into_int_value();

        let parent_fn = self.current_function()?;

        let then_bb = self.context.append_basic_block(parent_fn, "then");

        let has_else = children.get(2).and_then(|x| *x).is_some();

        let else_bb = if has_else {
            Some(self.context.append_basic_block(parent_fn, "else"))
        } else {
            None
        };

        let no_else_merge_bb = if has_else {
            None
        } else {
            Some(self.context.append_basic_block(parent_fn, "ifcont"))
        };

        if let Some(merge_bb) = no_else_merge_bb {
            self.builder
                .build_conditional_branch(cond_val, then_bb, merge_bb)
                .map_err(|e| e.to_string())?;
        } else {
            let else_bb = else_bb.ok_or("Missing else block")?;
            self.builder
                .build_conditional_branch(cond_val, then_bb, else_bb)
                .map_err(|e| e.to_string())?;
        }

        // Then block
        self.builder.position_at_end(then_bb);
        if let Some(then_id) = children[1] {
            self.execute(then_id)?;
        }
        let then_end_bb = self.builder.get_insert_block().ok_or("No insert block")?;
        let then_has_terminator = then_end_bb.get_terminator().is_some();

        if !has_else {
            if !then_has_terminator {
                let merge_bb = no_else_merge_bb.ok_or("Missing if merge block")?;
                self.builder
                    .build_unconditional_branch(merge_bb)
                    .map_err(|e| e.to_string())?;
            }
            let merge_bb = no_else_merge_bb.ok_or("Missing if merge block")?;
            self.builder.position_at_end(merge_bb);
            return Ok(());
        }

        // Else block
        let else_bb = else_bb.ok_or("Missing else block")?;
        self.builder.position_at_end(else_bb);
        if let Some(else_id) = children[2] {
            self.execute(else_id)?;
        }
        let else_end_bb = self.builder.get_insert_block().ok_or("No insert block")?;
        let else_has_terminator = else_end_bb.get_terminator().is_some();

        if then_has_terminator && else_has_terminator {
            return Ok(());
        }

        let merge_bb = self.context.append_basic_block(parent_fn, "ifcont");
        if !then_has_terminator {
            self.builder.position_at_end(then_end_bb);
            self.builder
                .build_unconditional_branch(merge_bb)
                .map_err(|e| e.to_string())?;
        }
        if !else_has_terminator {
            self.builder.position_at_end(else_end_bb);
            self.builder
                .build_unconditional_branch(merge_bb)
                .map_err(|e| e.to_string())?;
        }

        self.builder.position_at_end(merge_bb);
        Ok(())
    }

    pub(crate) fn visit_while(&mut self, id: NodeId) -> Result<(), String> {
        let children = self.arena[id].children.clone();

        let parent_fn = self.current_function()?;

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

    pub(crate) fn visit_logical(&mut self, id: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let op_type = self.arena[id].token.token_type;
        let children = self.arena[id].children.clone();

        let left_id = children[0].expect("Missing left operand");
        let right_id = children[1].expect("Missing right operand");

        let left_val = self.evaluate(left_id)?.into_int_value();
        let left_bb = self.builder.get_insert_block().ok_or("No insert block")?;

        let parent_fn = self.current_function()?;
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

    pub(crate) fn visit_return(&mut self, id: NodeId) -> Result<(), String> {
        let children = self.arena[id].children.clone();
        let current_fn = self.current_function()?;

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
}
