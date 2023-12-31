use std::collections::HashMap;

use log::{debug, info};

use crate::{
    errors::SyntaxError,
    tokenizer::{Keyword, Token},
    variables::{Operator, VariableValue},
};

#[derive(Debug, Clone)]
pub enum Statement {
    VariableDefinition(String, Expression),
    VariableAssignment(ReferenceExpr, Expression),
    Expr(Expression),
    Return(Expression),
    Break(Expression),
    Continue,
    ImplicitReturn(Expression),
}

#[derive(Debug, Clone)]
pub enum Expression {
    Value(VariableValue),
    List(Vec<Expression>),
    Object(HashMap<String, Expression>),
    Reference(Box<ReferenceExpr>),
    BinaryOperator(Box<Expression>, Box<Expression>, Operator),
    UnaryOperator(Box<Expression>, Operator),
    Block(Vec<Statement>),
    FunctionCall(Box<Expression>, Vec<Expression>),
    BuiltinFunctionCall(String, Option<VariableValue>, Vec<VariableValue>),
    IfElse(Box<Expression>, Box<Expression>, Option<Box<Expression>>),
    ForLoop(String, Box<Expression>, Box<Expression>),
    WhileLoop(Box<Expression>, Box<Expression>),
}

#[derive(Debug, Clone)]
pub enum ReferenceExpr {
    Variable(String),
    Index(Expression, Expression),
    Object(Expression, String),
}

#[derive(Debug)]
pub enum PartialParsed {
    Token(Token),
    Braces(Vec<PartialParsed>),
    Parentheses(Vec<PartialParsed>),
    Brackets(Vec<PartialParsed>),
    Closure(Vec<String>),
}

pub fn reduce_brackets_and_parenths(t: &[Token]) -> Result<Vec<PartialParsed>, SyntaxError> {
    let mut reduced_t = Vec::new();

    let mut i = 0;
    while i < t.len() {
        match t.get(i) {
            Some(Token::OpeningParethesis) => {
                let closing = find_matching(&t[i + 1..], |tkn| {
                    matches!(tkn, Some(Token::ClosingParethesis))
                })
                .ok_or(SyntaxError::from("No matching closing parenthesis!"))?;
                let reduced = reduce_brackets_and_parenths(&t[i + 1..i + 1 + closing])?;
                reduced_t.push(PartialParsed::Parentheses(reduced));
                i += closing + 1;
            }
            Some(Token::OpeningBrace) => {
                let closing =
                    find_matching(&t[i + 1..], |tkn| matches!(tkn, Some(Token::ClosingBrace)))
                        .ok_or(SyntaxError::from("No matching closing brace!"))?;
                let reduced = reduce_brackets_and_parenths(&t[i + 1..i + 1 + closing])?;
                reduced_t.push(PartialParsed::Braces(reduced));
                i += closing + 1;
            }
            Some(Token::OpeningBracket) => {
                let closing = find_matching(&t[i + 1..], |tkn| {
                    matches!(tkn, Some(Token::ClosingBracket))
                })
                .ok_or(SyntaxError::from("No matching closing bracket!"))?;
                let reduced = reduce_brackets_and_parenths(&t[i + 1..i + 1 + closing])?;
                reduced_t.push(PartialParsed::Brackets(reduced));
                i += closing + 1;
            }
            Some(Token::VerticalBar) => {
                let closing =
                    find_matching(&t[i + 1..], |tkn| matches!(tkn, Some(Token::VerticalBar)))
                        .ok_or(SyntaxError::from("No matching closing bracket!"))?;
                let reduced = get_comma_separated_identifiers(&t[i + 1..i + 1 + closing])?;
                reduced_t.push(PartialParsed::Closure(reduced));
                i += closing + 1;
            }
            Some(tkn) => reduced_t.push(PartialParsed::Token(tkn.clone())),
            None => (),
        }
        i += 1;
    }

    Ok(reduced_t)
}

pub fn get_statements(t: &[PartialParsed]) -> Result<Vec<Statement>, SyntaxError> {
    let mut statements = Vec::new();
    let semis: Vec<usize> = t
        .iter()
        .enumerate()
        .filter_map(|(i, tkn)| {
            if matches!(tkn, PartialParsed::Token(Token::Semicolon)) {
                Some(i)
            } else {
                None
            }
        })
        .collect();
    for i in 0..=semis.len() {
        let start = if i == 0 { 0 } else { semis[i - 1] + 1 };
        let end = if i == semis.len() { t.len() } else { semis[i] };
        if end - start > 0 {
            let stmnt = get_stmnt(&t[start..end])?;

            info!("statement: {:?}", stmnt);
            statements.push(stmnt);
        }
    }

    if let Some(PartialParsed::Token(Token::Semicolon)) = t.last() {
        return Ok(statements);
    }

    if let Some(last_stmnt) = statements.last_mut() {
        if let Statement::Expr(ref last_expr) = last_stmnt {
            *last_stmnt = Statement::ImplicitReturn(last_expr.clone());
        }
        Ok(statements)
    } else {
        Err("empty block!".into())
    }
}

pub fn get_stmnt(t: &[PartialParsed]) -> Result<Statement, SyntaxError> {
    if let Some(PartialParsed::Token(Token::Keyword(Keyword::Let))) = t.get(0) {
        if let (
            Some(PartialParsed::Token(Token::Identifier(var_name))),
            Some(PartialParsed::Token(Token::Assign)),
        ) = (t.get(1), t.get(2))
        {
            let expr = get_expr(&t[3..])?;
            return Ok(Statement::VariableDefinition(var_name.to_string(), expr));
        } else {
            return Err("Invalid VariableDefinition Statement".into());
        }
    }

    if let Some(PartialParsed::Token(Token::Keyword(Keyword::Return))) = t.get(0) {
        return if t.len() > 1 {
            let expr = get_expr(&t[1..])?;
            Ok(Statement::Return(expr))
        } else {
            Ok(Statement::Return(Expression::Value(VariableValue::Unit)))
        };
    }

    if let Some(PartialParsed::Token(Token::Keyword(Keyword::Break))) = t.get(0) {
        return if t.len() > 1 {
            let expr = get_expr(&t[1..])?;
            Ok(Statement::Break(expr))
        } else {
            Ok(Statement::Break(Expression::Value(VariableValue::Unit)))
        };
    }

    if let Some(PartialParsed::Token(Token::Keyword(Keyword::Continue))) = t.get(0) {
        return if t.len() > 1 {
            Err(SyntaxError("invalid statement after continue".into()))
        } else {
            Ok(Statement::Continue)
        };
    }

    if let Some(i) = t
        .iter()
        .position(|tkn| matches!(tkn, PartialParsed::Token(Token::Assign)))
    {
        let expr = get_expr(&t[..i])?;
        let val_expr = get_expr(&t[i + 1..])?;
        if let Expression::Reference(ref_expr) = expr {
            return Ok(Statement::VariableAssignment(*ref_expr, val_expr));
        } else {
            return Err("can only assign to a reference".into());
        }
    }

    if let Some((i, op)) = t.iter().enumerate().find_map(|(i, tkn)| {
        if let PartialParsed::Token(Token::OperatorAssign(op)) = tkn {
            Some((i, op))
        } else {
            None
        }
    }) {
        let expr = get_expr(&t[..i])?;
        let val_expr = get_expr(&t[i + 1..])?;
        if let Expression::Reference(ref ref_expr) = expr {
            return Ok(Statement::VariableAssignment(
                *ref_expr.clone(),
                Expression::BinaryOperator(Box::new(expr), Box::new(val_expr), op.clone()),
            ));
        } else {
            return Err("can only assign to a reference".into());
        }
    }

    get_expr(t).map(|e| Statement::Expr(e))
}

pub fn get_expr(t: &[PartialParsed]) -> Result<Expression, SyntaxError> {
    debug!("get expr: {:?}", t);
    if t.len() == 0 {
        return Err("Empty expr".into());
    }
    if t.len() == 1 {
        return match t[0] {
            PartialParsed::Braces(ref b) => {
                if b.len() == 0 {
                    Ok(Expression::Value(VariableValue::Object(HashMap::new())))
                } else if b
                    .iter()
                    .any(|v| matches!(v, PartialParsed::Token(Token::Colon)))
                {
                    get_object(b)
                } else {
                    get_statements(b).map(|v| Expression::Block(v))
                }
            }
            PartialParsed::Parentheses(ref b) => {
                if b.len() == 0 {
                    Ok(Expression::Value(VariableValue::Unit))
                } else {
                    get_expr(b)
                }
            }
            PartialParsed::Brackets(ref b) => if b.len() > 0 {
                get_comma_separated_exprs(b)
            } else {
                Ok(Vec::new())
            }
            .map(|v| Expression::List(v)),
            PartialParsed::Token(ref b) => match b {
                Token::Value(v) => Ok(Expression::Value(v.clone())),
                Token::Identifier(v) => Ok(Expression::Reference(Box::new(
                    ReferenceExpr::Variable(v.to_string()),
                ))),
                _ => Err("Not a valid token expr".into()),
            },
            _ => Err("Not a valid alone closure".into()),
        };
    }

    let is_closure = t.iter().any(|tkn| matches!(tkn, PartialParsed::Closure(_)));
    if is_closure {
        if let Some(PartialParsed::Closure(args)) = t.get(0) {
            let expr = get_expr(&t[1..])?;

            return Ok(Expression::Value(VariableValue::Function(
                args.to_vec(),
                Box::new(expr),
            )));
        } else {
            return Err(format!("invalid closure expresion: {:?}", t).into());
        }
    }

    let is_for_loop = t
        .iter()
        .any(|tkn| matches!(tkn, PartialParsed::Token(Token::Keyword(Keyword::For))));
    if is_for_loop {
        if let (
            Some(PartialParsed::Token(Token::Keyword(Keyword::For))),
            Some(PartialParsed::Token(Token::Identifier(var_name))),
            Some(PartialParsed::Token(Token::Keyword(Keyword::In))),
        ) = (t.get(0), t.get(1), t.get(2))
        {
            if t.len() < 5 {
                return Err(format!("invalid for loop: {:?}", t).into());
            }
            let iterator = get_expr(&t[3..t.len() - 1])?;
            let body_expr = get_expr(&t[t.len() - 1..])?;
            return match body_expr {
                Expression::Block(body) => Ok(Expression::ForLoop(
                    var_name.to_string(),
                    Box::new(iterator),
                    Box::new(Expression::Block(body)),
                )),
                Expression::Value(VariableValue::Object(map)) if map.len() == 0 => {
                    Ok(Expression::ForLoop(
                        var_name.to_string(),
                        Box::new(iterator),
                        Box::new(Expression::Block(Vec::new())),
                    ))
                }
                _ => Err(format!("invalid for loop body: {:?}", t).into()),
            };
        } else {
            return Err(format!("invalid for loop: {:?}", t).into());
        }
    }

    let is_while_loop = t
        .iter()
        .any(|tkn| matches!(tkn, PartialParsed::Token(Token::Keyword(Keyword::While))));
    if is_while_loop {
        if let Some(PartialParsed::Token(Token::Keyword(Keyword::While))) = t.get(0) {
            if t.len() < 3 {
                return Err(format!("invalid while loop: {:?}", t).into());
            }
            let condition = get_expr(&t[1..t.len() - 1])?;
            let body_expr = get_expr(&t[t.len() - 1..])?;
            return match body_expr {
                Expression::Block(body) => Ok(Expression::WhileLoop(
                    Box::new(condition),
                    Box::new(Expression::Block(body)),
                )),
                Expression::Value(VariableValue::Object(map)) if map.len() == 0 => {
                    Ok(Expression::WhileLoop(
                        Box::new(condition),
                        Box::new(Expression::Block(Vec::new())),
                    ))
                }
                _ => Err(format!("invalid while loop body: {:?}", t).into()),
            };
        } else {
            return Err(format!("invalid while loop: {:?}", t).into());
        }
    }

    let if_pos = t
        .iter()
        .position(|tkn| matches!(tkn, PartialParsed::Token(Token::Keyword(Keyword::If))));
    if let Some(if_i) = if_pos {
        if let Some(else_pos) = t
            .iter()
            .position(|tkn| matches!(tkn, PartialParsed::Token(Token::Keyword(Keyword::Else))))
        {
            if t.len() < 5 {
                return Err(format!("invalid if statement: {:?}", t).into());
            }
            let cond = get_expr(&t[if_i + 1..else_pos - 1])?;

            if let (Expression::Block(e1), Expression::Block(e2)) = (
                get_expr(&t[else_pos - 1..else_pos])?,
                get_expr(&t[else_pos + 1..else_pos + 2])?,
            ) {
                return Ok(Expression::IfElse(
                    Box::new(cond),
                    Box::new(Expression::Block(e1)),
                    Some(Box::new(Expression::Block(e2))),
                ));
            } else {
                return Err(format!("invalid if else body: {:?}", t).into());
            }
        } else {
            if t.len() < 3 {
                return Err(format!("invalid if statement: {:?}", t).into());
            }
            let cond = get_expr(&t[if_i + 1..t.len() - 1])?;

            if let Expression::Block(e1) = get_expr(&t[t.len() - 1..])? {
                return Ok(Expression::IfElse(
                    Box::new(cond),
                    Box::new(Expression::Block(e1)),
                    None,
                ));
            } else {
                return Err(format!("invalid if body: {:?}", t).into());
            }
        }
    }

    let mut lowest_op = None;
    let mut lowest_precedence = None;
    for i in 0..t.len() {
        match t[i] {
            PartialParsed::Token(Token::Operator(ref op)) => {
                let prec = op.precedence();
                if !lowest_precedence.is_some_and(|v| v < prec) {
                    lowest_precedence = Some(prec);
                    lowest_op = Some((i, op.clone()));
                }
            }
            _ => (),
        }
    }

    if let Some((i, op)) = lowest_op {
        if i == 0 {
            let e = get_expr(&t[i + 1..])?;
            let unary_op = match op {
                Operator::Add => Ok(Operator::UnaryPlus),
                Operator::Subtract => Ok(Operator::Negate),
                Operator::Not => Ok(Operator::Not),
                _ => Err(SyntaxError::from("no such unary operator")),
            }?;
            return Ok(Expression::UnaryOperator(Box::new(e), unary_op));
        } else {
            let e1 = get_expr(&t[..i])?;
            let e2 = get_expr(&t[i + 1..])?;

            return Ok(Expression::BinaryOperator(Box::new(e1), Box::new(e2), op));
        }
    }

    if let Some(PartialParsed::Parentheses(p)) = t.last() {
        let fun = get_expr(&t[..t.len() - 1])?;
        let args = if p.len() > 0 {
            get_comma_separated_exprs(p)?
        } else {
            Vec::new()
        };
        return Ok(Expression::FunctionCall(Box::new(fun), args));
    }

    if let Some(PartialParsed::Brackets(p)) = t.last() {
        let fun = get_expr(&t[..t.len() - 1])?;
        let index = get_expr(p)?;
        return Ok(Expression::Reference(Box::new(ReferenceExpr::Index(
            fun, index,
        ))));
    }

    if let (
        Some(PartialParsed::Token(Token::Dot)),
        Some(PartialParsed::Token(Token::Identifier(var_name))),
    ) = (t.get(t.len() - 2), t.last())
    {
        let fun = get_expr(&t[..t.len() - 2])?;
        return Ok(Expression::Reference(Box::new(ReferenceExpr::Object(
            fun,
            var_name.to_string(),
        ))));
    }

    Err(format!("Not a valid expr: {:?}. Are you missing a semicolon?", t).into())
}

pub fn get_object(t: &[PartialParsed]) -> Result<Expression, SyntaxError> {
    let commas: Vec<usize> = t
        .iter()
        .enumerate()
        .filter_map(|(i, tkn)| {
            if matches!(tkn, PartialParsed::Token(Token::Comma)) {
                Some(i)
            } else {
                None
            }
        })
        .collect();
    let mut exprs = HashMap::with_capacity(commas.len() + 1);
    for i in 0..=commas.len() {
        let start = if i == 0 { 0 } else { commas[i - 1] + 1 };
        let end = if i == commas.len() {
            t.len()
        } else {
            commas[i]
        };

        if let (
            PartialParsed::Token(Token::Identifier(ref var_name)),
            PartialParsed::Token(Token::Colon),
        ) = (&t[start], &t[start + 1])
        {
            let expr = get_expr(&t[start + 2..end])?;
            exprs.insert(var_name.to_string(), expr);
        } else {
            return Err("invalid variable name".into());
        }
    }
    Ok(Expression::Object(exprs))
}

pub fn get_comma_separated_exprs(t: &[PartialParsed]) -> Result<Vec<Expression>, SyntaxError> {
    let mut exprs = Vec::new();
    let commas: Vec<usize> = t
        .iter()
        .enumerate()
        .filter_map(|(i, tkn)| {
            if matches!(tkn, PartialParsed::Token(Token::Comma)) {
                Some(i)
            } else {
                None
            }
        })
        .collect();
    for i in 0..=commas.len() {
        let start = if i == 0 { 0 } else { commas[i - 1] + 1 };
        let end = if i == commas.len() {
            t.len()
        } else {
            commas[i]
        };
        let expr = get_expr(&t[start..end])?;
        exprs.push(expr);
    }
    Ok(exprs)
}

pub fn get_comma_separated_identifiers(t: &[Token]) -> Result<Vec<String>, SyntaxError> {
    let mut idents = Vec::new();
    let commas: Vec<usize> = t
        .iter()
        .enumerate()
        .filter_map(|(i, tkn)| {
            if matches!(tkn, Token::Comma) {
                Some(i)
            } else {
                None
            }
        })
        .collect();
    for i in 0..=commas.len() {
        let start = if i == 0 { 0 } else { commas[i - 1] + 1 };
        let end = if i == commas.len() {
            t.len()
        } else {
            commas[i]
        };
        if start + 1 == end {
            if let Token::Identifier(ref n) = t[start] {
                idents.push(n.to_string());
            } else {
                return Err("invalid identifier in list".into());
            }
        } else {
            return Err("invalid identifier list".into());
        }
    }
    Ok(idents)
}

fn find_matching<T>(t: &[Token], matching: T) -> Option<usize>
where
    T: Fn(Option<&Token>) -> bool,
{
    let mut indent_level = 0;
    for i in 0..t.len() {
        if indent_level == 0 && matching(t.get(i)) {
            return Some(i);
        }
        match t[i] {
            Token::OpeningParethesis => indent_level += 1,
            Token::OpeningBrace => indent_level += 1,
            Token::OpeningBracket => indent_level += 1,
            Token::ClosingParethesis => indent_level -= 1,
            Token::ClosingBrace => indent_level -= 1,
            Token::ClosingBracket => indent_level -= 1,
            _ => (),
        }
    }
    None
}
