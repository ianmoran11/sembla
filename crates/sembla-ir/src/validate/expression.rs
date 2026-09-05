use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ValueType {
    Real,
    Int,
    Bool,
    Enum(Vec<String>),
    Ref(String),
}

impl ValueType {
    pub(super) fn name(&self) -> &'static str {
        match self {
            Self::Real => "Real",
            Self::Int => "Int",
            Self::Bool => "Bool",
            Self::Enum(_) => "Enum",
            Self::Ref(_) => "Ref",
        }
    }

    pub(super) fn is_numeric(&self) -> bool {
        matches!(self, Self::Real | Self::Int)
    }

    pub(super) fn is_orderable(&self) -> bool {
        matches!(self, Self::Real | Self::Int | Self::Enum(_))
    }
}

impl From<&AttrType> for ValueType {
    fn from(value: &AttrType) -> Self {
        match value {
            AttrType::Real => Self::Real,
            AttrType::Int => Self::Int,
            AttrType::Enum { variants } => Self::Enum(variants.clone()),
            AttrType::Ref { table } => Self::Ref(table.clone()),
        }
    }
}

impl From<ParamType> for ValueType {
    fn from(value: ParamType) -> Self {
        match value {
            ParamType::Real => Self::Real,
            ParamType::Int => Self::Int,
        }
    }
}

fn validate_input_row_expr(expr: &Expr, path: &str) -> Result<(), ValidationError> {
    match expr {
        Expr::Input { .. } | Expr::Agg { .. } => Err(error(
            path,
            "nested aggregates are not supported inside input table aggregates",
        )),
        Expr::Add { lhs, rhs }
        | Expr::Sub { lhs, rhs }
        | Expr::Mul { lhs, rhs }
        | Expr::Div { lhs, rhs }
        | Expr::Eq { lhs, rhs }
        | Expr::Ne { lhs, rhs }
        | Expr::Lt { lhs, rhs }
        | Expr::Le { lhs, rhs }
        | Expr::Gt { lhs, rhs }
        | Expr::Ge { lhs, rhs }
        | Expr::And { lhs, rhs }
        | Expr::Or { lhs, rhs } => {
            validate_input_row_expr(lhs, &format!("{path}.lhs"))?;
            validate_input_row_expr(rhs, &format!("{path}.rhs"))
        }
        Expr::Not { expr } => validate_input_row_expr(expr, &format!("{path}.expr")),
        Expr::Real { .. }
        | Expr::Int { .. }
        | Expr::Bool { .. }
        | Expr::Enum { .. }
        | Expr::Param { .. }
        | Expr::SelfAttr { .. }
        | Expr::EnumIs { .. } => Ok(()),
    }
}

pub(super) fn infer_expr(
    expr: &Expr,
    model: &Model,
    model_box: &Box,
    row_attrs: &[Attr],
    path: &str,
    expected: Option<&ValueType>,
) -> Result<ValueType, ValidationError> {
    match expr {
        Expr::Real { value } => {
            if !value.is_finite() {
                Err(error(
                    format!("{path}.value"),
                    "real literal must be finite",
                ))
            } else {
                Ok(ValueType::Real)
            }
        }
        Expr::Int { .. } => Ok(ValueType::Int),
        Expr::Bool { .. } => Ok(ValueType::Bool),
        Expr::Enum { variant } => match expected {
            Some(ValueType::Enum(variants)) => {
                if variants.contains(variant) {
                    Ok(ValueType::Enum(variants.clone()))
                } else {
                    Err(error(
                        format!("{path}.variant"),
                        format!("unknown enum variant '{variant}'"),
                    ))
                }
            }
            _ => Err(error(
                path,
                format!("enum literal '{variant}' requires an Enum-typed context"),
            )),
        },
        Expr::Param { name } => model
            .params
            .iter()
            .find(|param| param.name == *name)
            .map(|param| ValueType::from(param.ty))
            .ok_or_else(|| {
                error(
                    format!("{path}.name"),
                    format!("unresolved parameter '{name}'"),
                )
            }),
        Expr::SelfAttr { name } => find_attr(row_attrs, name)
            .map(|attr| ValueType::from(&attr.ty))
            .ok_or_else(|| {
                error(
                    format!("{path}.name"),
                    format!("unresolved self attribute '{name}'"),
                )
            }),
        Expr::Add { lhs, rhs } | Expr::Sub { lhs, rhs } | Expr::Mul { lhs, rhs } => {
            infer_numeric_binary(lhs, rhs, model, model_box, row_attrs, path, false)
        }
        Expr::Div { lhs, rhs } => {
            infer_numeric_binary(lhs, rhs, model, model_box, row_attrs, path, true)
        }
        Expr::Eq { lhs, rhs } | Expr::Ne { lhs, rhs } => {
            infer_equality(lhs, rhs, model, model_box, row_attrs, path)
        }
        Expr::Lt { lhs, rhs }
        | Expr::Le { lhs, rhs }
        | Expr::Gt { lhs, rhs }
        | Expr::Ge { lhs, rhs } => {
            let lhs_type = infer_expr(
                lhs,
                model,
                model_box,
                row_attrs,
                &format!("{path}.lhs"),
                None,
            )?;
            let rhs_type = infer_expr(
                rhs,
                model,
                model_box,
                row_attrs,
                &format!("{path}.rhs"),
                Some(&lhs_type),
            )?;
            if !(lhs_type.is_numeric() && rhs_type.is_numeric()) {
                return Err(error(path, "ordered comparison operands must be numeric"));
            }
            Ok(ValueType::Bool)
        }
        Expr::And { lhs, rhs } | Expr::Or { lhs, rhs } => {
            require_expr_type(
                lhs,
                model,
                model_box,
                row_attrs,
                &format!("{path}.lhs"),
                &ValueType::Bool,
            )?;
            require_expr_type(
                rhs,
                model,
                model_box,
                row_attrs,
                &format!("{path}.rhs"),
                &ValueType::Bool,
            )?;
            Ok(ValueType::Bool)
        }
        Expr::Not { expr } => {
            require_expr_type(
                expr,
                model,
                model_box,
                row_attrs,
                &format!("{path}.expr"),
                &ValueType::Bool,
            )?;
            Ok(ValueType::Bool)
        }
        Expr::EnumIs { attr, variant } => {
            let declaration = find_attr(row_attrs, attr).ok_or_else(|| {
                error(
                    format!("{path}.attr"),
                    format!("EnumIs refers to unknown attribute '{attr}'"),
                )
            })?;
            match &declaration.ty {
                AttrType::Enum { variants } if variants.contains(variant) => Ok(ValueType::Bool),
                AttrType::Enum { .. } => Err(error(
                    format!("{path}.variant"),
                    format!("unknown variant '{variant}' for enum attribute '{attr}'"),
                )),
                _ => Err(error(
                    format!("{path}.attr"),
                    format!("EnumIs attribute '{attr}' is not Enum-typed"),
                )),
            }
        }
        Expr::Input { port, agg } => {
            let input = model_box
                .inputs
                .iter()
                .find(|input| input.name == *port)
                .ok_or_else(|| {
                    error(
                        format!("{path}.port"),
                        format!("unresolved input port '{port}'"),
                    )
                })?;
            if let Some(filter) = &agg.filter {
                validate_input_row_expr(filter, &format!("{path}.agg.filter"))?;
                require_expr_type(
                    filter,
                    model,
                    model_box,
                    &input.schema,
                    &format!("{path}.agg.filter"),
                    &ValueType::Bool,
                )?;
            }
            if let AggOp::Sum { value } = &agg.op {
                validate_input_row_expr(value, &format!("{path}.agg.op.value"))?;
            }
            infer_agg_op(
                &agg.op,
                model,
                model_box,
                &input.schema,
                &format!("{path}.agg.op"),
            )
        }
        Expr::Agg {
            op,
            table,
            on,
            filter,
        } => {
            let target = find_table(model_box, table).ok_or_else(|| {
                error(
                    format!("{path}.table"),
                    format!("aggregate refers to unknown table '{table}'"),
                )
            })?;
            let target_fk = find_attr(&target.attrs, &on.fk_attr).ok_or_else(|| {
                error(
                    format!("{path}.on.fk_attr"),
                    format!(
                        "aggregate table '{}' has no attribute '{}'",
                        target.name, on.fk_attr
                    ),
                )
            })?;
            let self_fk = find_attr(row_attrs, &on.self_fk_attr).ok_or_else(|| {
                error(
                    format!("{path}.on.self_fk_attr"),
                    format!("current row has no attribute '{}'", on.self_fk_attr),
                )
            })?;
            match (&target_fk.ty, &self_fk.ty) {
                (AttrType::Ref { table: target_ref }, AttrType::Ref { table: self_ref })
                    if target_ref == self_ref => {}
                _ => {
                    return Err(error(
                        format!("{path}.on"),
                        "aggregate join attributes must both be Ref attributes to the same table",
                    ));
                }
            }
            require_expr_type(
                filter,
                model,
                model_box,
                &target.attrs,
                &format!("{path}.filter"),
                &ValueType::Bool,
            )?;
            infer_agg_op(op, model, model_box, &target.attrs, &format!("{path}.op"))
        }
    }
}

pub(super) fn infer_agg_op(
    op: &AggOp,
    model: &Model,
    model_box: &Box,
    row_attrs: &[Attr],
    path: &str,
) -> Result<ValueType, ValidationError> {
    match op {
        AggOp::Count => Ok(ValueType::Int),
        AggOp::Sum { value } => {
            let value_type = infer_expr(
                value,
                model,
                model_box,
                row_attrs,
                &format!("{path}.value"),
                None,
            )?;
            if !value_type.is_numeric() {
                return Err(error(
                    format!("{path}.value"),
                    format!("Sum value must be numeric, found {}", value_type.name()),
                ));
            }
            Ok(value_type)
        }
    }
}

fn infer_numeric_binary(
    lhs: &Expr,
    rhs: &Expr,
    model: &Model,
    model_box: &Box,
    row_attrs: &[Attr],
    path: &str,
    division: bool,
) -> Result<ValueType, ValidationError> {
    let lhs_type = infer_expr(
        lhs,
        model,
        model_box,
        row_attrs,
        &format!("{path}.lhs"),
        None,
    )?;
    let rhs_type = infer_expr(
        rhs,
        model,
        model_box,
        row_attrs,
        &format!("{path}.rhs"),
        None,
    )?;
    if !(lhs_type.is_numeric() && rhs_type.is_numeric()) {
        return Err(error(path, "arithmetic operands must be Real or Int"));
    }
    if division || lhs_type == ValueType::Real || rhs_type == ValueType::Real {
        Ok(ValueType::Real)
    } else {
        Ok(ValueType::Int)
    }
}

fn infer_equality(
    lhs: &Expr,
    rhs: &Expr,
    model: &Model,
    model_box: &Box,
    row_attrs: &[Attr],
    path: &str,
) -> Result<ValueType, ValidationError> {
    let (lhs_type, rhs_type) = if matches!(lhs, Expr::Enum { .. }) {
        let rhs_type = infer_expr(
            rhs,
            model,
            model_box,
            row_attrs,
            &format!("{path}.rhs"),
            None,
        )?;
        let lhs_type = infer_expr(
            lhs,
            model,
            model_box,
            row_attrs,
            &format!("{path}.lhs"),
            Some(&rhs_type),
        )?;
        (lhs_type, rhs_type)
    } else {
        let lhs_type = infer_expr(
            lhs,
            model,
            model_box,
            row_attrs,
            &format!("{path}.lhs"),
            None,
        )?;
        let rhs_type = infer_expr(
            rhs,
            model,
            model_box,
            row_attrs,
            &format!("{path}.rhs"),
            Some(&lhs_type),
        )?;
        (lhs_type, rhs_type)
    };
    if lhs_type != rhs_type && !(lhs_type.is_numeric() && rhs_type.is_numeric()) {
        return Err(error(
            path,
            format!(
                "equality operands have incompatible types {} and {}",
                lhs_type.name(),
                rhs_type.name()
            ),
        ));
    }
    Ok(ValueType::Bool)
}

pub(super) fn require_type(
    actual: &ValueType,
    expected: &ValueType,
    path: &str,
) -> Result<(), ValidationError> {
    if actual == expected {
        Ok(())
    } else {
        Err(error(
            path,
            format!("expected {}, found {}", expected.name(), actual.name()),
        ))
    }
}

pub(super) fn require_expr_type(
    expr: &Expr,
    model: &Model,
    model_box: &Box,
    row_attrs: &[Attr],
    path: &str,
    expected: &ValueType,
) -> Result<(), ValidationError> {
    let actual = infer_expr(expr, model, model_box, row_attrs, path, Some(expected))?;
    require_type(&actual, expected, path)
}
