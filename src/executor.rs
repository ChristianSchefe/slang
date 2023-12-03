use std::collections::HashMap;

use crate::*;

pub fn exec_stmnts(
    variables: &mut HashMap<String, VariableValue>,
    stmnts: &[Statement],
) -> Result<VariableValue, RuntimeError> {
    for stmnt in stmnts {
        match exec_stmnt(variables, stmnt)? {
            VariableValue::Unit => (),
            result => return Ok(result),
        }
    }
    Ok(VariableValue::Unit)
}

pub fn exec_stmnt(
    variables: &mut HashMap<String, VariableValue>,
    stmnt: &Statement,
) -> Result<VariableValue, RuntimeError> {
    info!("exec: {:?}", stmnt);
    match stmnt {
        Statement::VariableDefinition(var, val) => define_var(variables, var, val),
        Statement::VariableAssignment(var, val) => assign_var(variables, var, val),
        Statement::Expr(expr) => eval_expr(variables, expr).map(|_| VariableValue::Unit),
    }
}

pub fn eval_expr(
    variables: &mut HashMap<String, VariableValue>,
    expr: &Expression,
) -> Result<VariableValue, RuntimeError> {
    match expr {
        Expression::List(list) => Ok(VariableValue::List(
            list.iter()
                .map(|v| eval_expr(variables, v))
                .collect::<Result<Vec<VariableValue>, RuntimeError>>()?,
        )),
        Expression::Object(fields) => Ok(VariableValue::Object(
            fields
                .iter()
                .map(|(key, v)| eval_expr(variables, v).map(|r| (key.clone(), r)))
                .collect::<Result<HashMap<String, VariableValue>, RuntimeError>>()?,
        )),
        Expression::Value(var) => Ok(var.clone()),
        Expression::Reference(ref_expr) => get_var_cloned(variables, ref_expr),
        Expression::FunctionCall(func_expr, params) => eval_expr(variables, func_expr)?.call(
            params
                .iter()
                .map(|v| eval_expr(variables, v))
                .collect::<Result<Vec<VariableValue>, RuntimeError>>()?,
        ),
        Expression::BinaryOperator(a, b, op) => {
            evaluate_binary_op(eval_expr(variables, a)?, eval_expr(variables, b)?, *op)
        }
        Expression::UnaryOperator(a, op) => evaluate_unary_op(eval_expr(variables, a)?, *op),
        Expression::IfElse(cond_expr, if_expr, else_expr) => {
            if let VariableValue::Boolean(cond) = eval_expr(variables, cond_expr)? {
                eval_expr(variables, if cond { if_expr } else { else_expr })
            } else {
                Err("condition is not a boolean".into())
            }
        }
        _ => Ok(VariableValue::Unit),
    }
}

pub fn define_var(
    variables: &mut HashMap<String, VariableValue>,
    var: &str,
    expr: &Expression,
) -> Result<VariableValue, RuntimeError> {
    let val = eval_expr(variables, expr)?;
    define_var_by_val(variables, var, val)
}

pub fn define_var_by_val(
    variables: &mut HashMap<String, VariableValue>,
    var: &str,
    val: VariableValue,
) -> Result<VariableValue, RuntimeError> {
    if variables.contains_key(var) {
        Err("Variable is already defined".into())
    } else {
        variables.insert(var.to_string(), val);
        Ok(VariableValue::Unit)
    }
}

pub fn get_var<'a>(
    variables: &'a mut HashMap<String, VariableValue>,
    var_expr: &ReferenceExpr,
) -> Result<&'a mut VariableValue, RuntimeError> {
    match var_expr {
        ReferenceExpr::Variable(ref var) => {
            if let Some(val) = variables.get_mut(var) {
                Ok(val)
            } else {
                Err(format!("Variable '{}' is not defined", var).into())
            }
        }
        ReferenceExpr::Index(list_expr, index_expr) => {
            let index = eval_expr(variables, index_expr)?;
            if let Expression::Reference(ref_expr) = list_expr {
                let li = get_var(variables, ref_expr)?;
                if let (VariableValue::List(ref mut li), VariableValue::Number(i)) = (li, index) {
                    if let Some(val) = li.get_mut(i as usize) {
                        Ok(val)
                    } else {
                        Err("Index is out of bounds".into())
                    }
                } else {
                    Err("Variable is not a list".into())
                }
            } else {
                Err("Variable is not a ref".into())
            }
        }
        ReferenceExpr::Object(object_expr, index_expr) => {
            if let Expression::Reference(ref_expr) = object_expr {
                let object = get_var(variables, ref_expr)?;
                if let VariableValue::Object(ref mut obj) = object {
                    if let Some(val) = obj.get_mut(index_expr) {
                        Ok(val)
                    } else {
                        Err("Not a field of the object".into())
                    }
                } else {
                    Err("Variable is not an object".into())
                }
            } else {
                Err("Variable is not a ref".into())
            }
        }
    }
}
pub fn get_var_cloned(
    variables: &mut HashMap<String, VariableValue>,
    var_expr: &ReferenceExpr,
) -> Result<VariableValue, RuntimeError> {
    match var_expr {
        ReferenceExpr::Variable(ref var) => {
            if let Some(val) = variables.get(var) {
                Ok(val.clone())
            } else {
                Err(format!("Variable '{}' is not defined", var).into())
            }
        }
        ReferenceExpr::Index(list_expr, index_expr) => {
            let index = eval_expr(variables, index_expr)?;
            if let Expression::Reference(ref_expr) = list_expr {
                let li = get_var_cloned(variables, ref_expr)?;
                if let (VariableValue::List(ref li), VariableValue::Number(i)) = (li, index) {
                    if let Some(val) = li.get(i as usize) {
                        Ok(val.clone())
                    } else {
                        Err("Index is out of bounds".into())
                    }
                } else {
                    Err("Variable is not a list".into())
                }
            } else {
                let li = eval_expr(variables, list_expr)?;
                if let (VariableValue::List(ref li), VariableValue::Number(i)) = (li, index) {
                    if let Some(val) = li.get(i as usize) {
                        Ok(val.clone())
                    } else {
                        Err("Index is out of bounds".into())
                    }
                } else {
                    Err("Variable is not a list".into())
                }
            }
        }
        ReferenceExpr::Object(object_expr, index_expr) => {
            if let Expression::Reference(ref_expr) = object_expr {
                let object = get_var_cloned(variables, ref_expr)?;
                if let VariableValue::Object(ref obj) = object {
                    if let Some(val) = obj.get(index_expr) {
                        Ok(val.clone())
                    } else {
                        Err("Not a field of the object".into())
                    }
                } else {
                    Err("Variable is not an object".into())
                }
            } else {
                let object = eval_expr(variables, object_expr)?;
                if let VariableValue::Object(ref obj) = object {
                    if let Some(val) = obj.get(index_expr) {
                        Ok(val.clone())
                    } else {
                        Err("Not a field of the object".into())
                    }
                } else {
                    Err("Variable is not an object".into())
                }
            }
        }
    }
}

pub fn assign_var(
    variables: &mut HashMap<String, VariableValue>,
    var_expr: &ReferenceExpr,
    expr: &Expression,
) -> Result<VariableValue, RuntimeError> {
    let val = eval_expr(variables, expr)?;
    match var_expr {
        ReferenceExpr::Variable(ref var) => {
            if !variables.contains_key(var) {
                Err("Variable is not defined".into())
            } else {
                variables.insert(var.to_string(), val);
                Ok(VariableValue::Unit)
            }
        }
        ReferenceExpr::Index(list_expr, index_expr) => {
            let index = eval_expr(variables, index_expr)?;
            if let Expression::Reference(ref_expr) = list_expr {
                let list = get_var(variables, ref_expr)?;
                if let (VariableValue::List(ref mut vec), VariableValue::Number(i)) = (list, index)
                {
                    if let Some(mut_index) = vec.get_mut(i as usize) {
                        *mut_index = val;
                        Ok(VariableValue::Unit)
                    } else {
                        Err("Index is out of bound".into())
                    }
                } else {
                    Err("Variable is not a list".into())
                }
            } else {
                Err("Variable is not reference".into())
            }
        }
        ReferenceExpr::Object(object_expr, index_expr) => {
            if let Expression::Reference(ref_expr) = object_expr {
                let object = get_var(variables, ref_expr)?;
                if let VariableValue::Object(ref mut scope) = object {
                    if let Some(mut_index) = scope.get_mut(index_expr) {
                        *mut_index = val;
                        Ok(VariableValue::Unit)
                    } else {
                        Err("Not a field of the object".into())
                    }
                } else {
                    Err("Variable is not an object".into())
                }
            } else {
                Err("Variable is not a reference".into())
            }
        }
    }
}
