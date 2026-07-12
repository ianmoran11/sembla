use crate::model::*;
use crate::ValidationError;
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
enum ValueType {
    Real,
    Int,
    Bool,
    Enum(Vec<String>),
    Ref(String),
}

impl ValueType {
    fn name(&self) -> &'static str {
        match self {
            Self::Real => "Real",
            Self::Int => "Int",
            Self::Bool => "Bool",
            Self::Enum(_) => "Enum",
            Self::Ref(_) => "Ref",
        }
    }

    fn is_numeric(&self) -> bool {
        matches!(self, Self::Real | Self::Int)
    }

    fn is_orderable(&self) -> bool {
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

/// A transition annotated with its stable model-global declaration-order ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedTransition {
    pub box_index: usize,
    pub transition_index: usize,
    pub rule_id: u32,
}

/// A semantically valid model plus metadata derived during validation.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedModel {
    model: Model,
    transitions: Vec<ValidatedTransition>,
}

impl ValidatedModel {
    pub fn model(&self) -> &Model {
        &self.model
    }

    pub fn into_model(self) -> Model {
        self.model
    }

    pub fn transitions(&self) -> &[ValidatedTransition] {
        &self.transitions
    }

    pub fn rule_id(&self, box_index: usize, transition_index: usize) -> Option<u32> {
        self.transitions
            .iter()
            .find(|rule| rule.box_index == box_index && rule.transition_index == transition_index)
            .map(|rule| rule.rule_id)
    }
}

/// Validates all references and expression types, then assigns rule IDs.
pub fn validate(model: Model) -> Result<ValidatedModel, ValidationError> {
    validate_model(&model)?;

    let mut transitions = Vec::new();
    for (box_index, model_box) in model.boxes.iter().enumerate() {
        for transition_index in 0..model_box.transitions.len() {
            let rule_id = u32::try_from(transitions.len()).map_err(|_| {
                ValidationError::new(
                    format!("$.boxes[{box_index}].transitions[{transition_index}]"),
                    "too many transitions to assign a u32 rule_id",
                )
            })?;
            transitions.push(ValidatedTransition {
                box_index,
                transition_index,
                rule_id,
            });
        }
    }

    Ok(ValidatedModel { model, transitions })
}

fn validate_model(model: &Model) -> Result<(), ValidationError> {
    if !model.dt.is_finite() || model.dt <= 0.0 {
        return Err(error(
            "$.dt",
            "tick width must be finite and greater than zero",
        ));
    }

    unique_names(
        model.params.iter().map(|param| param.name.as_str()),
        "$.params",
        "parameter",
    )?;
    unique_names(
        model.boxes.iter().map(|model_box| model_box.name.as_str()),
        "$.boxes",
        "box",
    )?;

    for (index, param) in model.params.iter().enumerate() {
        validate_param(param, index)?;
    }
    for (index, model_box) in model.boxes.iter().enumerate() {
        validate_box(model, model_box, index)?;
    }
    for (index, wire) in model.wires.iter().enumerate() {
        if model.wires[..index]
            .iter()
            .any(|previous| previous.to.r#box == wire.to.r#box && previous.to.port == wire.to.port)
        {
            return Err(error(
                format!("$.wires[{index}].to"),
                format!(
                    "multiple wires target input '{}.{}'",
                    wire.to.r#box, wire.to.port
                ),
            ));
        }
        validate_wire(model, wire, index)?;
    }

    Ok(())
}

fn validate_param(param: &ParamDecl, index: usize) -> Result<(), ValidationError> {
    let base = format!("$.params[{index}]");
    let default_matches = matches!(
        (param.ty, &param.default),
        (ParamType::Real, ParamValue::Real { .. }) | (ParamType::Int, ParamValue::Int { .. })
    );
    if !default_matches {
        return Err(error(
            format!("{base}.default"),
            format!("default does not match parameter '{}' type", param.name),
        ));
    }
    if let ParamValue::Real { value } = param.default {
        if !value.is_finite() {
            return Err(error(
                format!("{base}.default.value"),
                format!("parameter '{}' default must be finite", param.name),
            ));
        }
    }

    if let Some(prior) = &param.prior {
        if prior.args.len() != 2 {
            return Err(error(
                format!("{base}.prior.args"),
                format!(
                    "parameter '{}' {:?} prior requires exactly 2 arguments, found {}",
                    param.name,
                    prior.family,
                    prior.args.len()
                ),
            ));
        }
        if prior.args.iter().any(|arg| !arg.is_finite()) {
            return Err(error(
                format!("{base}.prior.args"),
                format!("parameter '{}' prior arguments must be finite", param.name),
            ));
        }
        if prior.family == PriorFamily::Uniform && prior.args[0] >= prior.args[1] {
            return Err(error(
                format!("{base}.prior.args"),
                format!("parameter '{}' Uniform prior requires lo < hi", param.name),
            ));
        }
    }

    Ok(())
}

fn validate_box(model: &Model, model_box: &Box, box_index: usize) -> Result<(), ValidationError> {
    let base = format!("$.boxes[{box_index}]");
    unique_names(
        model_box.tables.iter().map(|table| table.name.as_str()),
        &format!("{base}.tables"),
        "table",
    )?;
    unique_names(
        model_box
            .transitions
            .iter()
            .map(|transition| transition.name.as_str()),
        &format!("{base}.transitions"),
        "transition",
    )?;
    unique_names(
        model_box.inputs.iter().map(|port| port.name.as_str()),
        &format!("{base}.inputs"),
        "input port",
    )?;
    unique_names(
        model_box.outputs.iter().map(|port| port.name.as_str()),
        &format!("{base}.outputs"),
        "output port",
    )?;

    for (table_index, table) in model_box.tables.iter().enumerate() {
        validate_schema(
            model_box,
            &table.attrs,
            &format!("{base}.tables[{table_index}].attrs"),
        )?;
    }
    for (port_index, port) in model_box.inputs.iter().enumerate() {
        validate_schema(
            model_box,
            &port.schema,
            &format!("{base}.inputs[{port_index}].schema"),
        )?;
    }
    for (output_index, output) in model_box.outputs.iter().enumerate() {
        let output_base = format!("{base}.outputs[{output_index}]");
        validate_schema(model_box, &output.schema, &format!("{output_base}.schema"))?;
        validate_output(model, model_box, output, &output_base)?;
    }
    for (transition_index, transition) in model_box.transitions.iter().enumerate() {
        validate_transition(
            model,
            model_box,
            transition,
            &format!("{base}.transitions[{transition_index}]"),
        )?;
    }

    Ok(())
}

fn validate_schema(model_box: &Box, attrs: &[Attr], path: &str) -> Result<(), ValidationError> {
    unique_names(
        attrs.iter().map(|attr| attr.name.as_str()),
        path,
        "attribute",
    )?;
    for (index, attr) in attrs.iter().enumerate() {
        match &attr.ty {
            AttrType::Enum { variants } => {
                if variants.is_empty() {
                    return Err(error(
                        format!("{path}[{index}].ty.variants"),
                        format!("enum attribute '{}' must declare a variant", attr.name),
                    ));
                }
                unique_names(
                    variants.iter().map(String::as_str),
                    &format!("{path}[{index}].ty.variants"),
                    "enum variant",
                )?;
            }
            AttrType::Ref { table } => {
                if find_table(model_box, table).is_none() {
                    return Err(error(
                        format!("{path}[{index}].ty.table"),
                        format!(
                            "attribute '{}' refers to unknown table '{table}'",
                            attr.name
                        ),
                    ));
                }
            }
            AttrType::Real | AttrType::Int => {}
        }
    }
    Ok(())
}

fn validate_transition(
    model: &Model,
    model_box: &Box,
    transition: &Transition,
    path: &str,
) -> Result<(), ValidationError> {
    let table = find_table(model_box, &transition.table).ok_or_else(|| {
        error(
            format!("{path}.table"),
            format!(
                "transition '{}' refers to unknown table '{}'",
                transition.name, transition.table
            ),
        )
    })?;

    let guard_type = infer_expr(
        &transition.guard,
        model,
        model_box,
        &table.attrs,
        &format!("{path}.guard"),
        Some(&ValueType::Bool),
    )?;
    require_type(&guard_type, &ValueType::Bool, &format!("{path}.guard"))?;

    let hazard_type = infer_expr(
        &transition.hazard,
        model,
        model_box,
        &table.attrs,
        &format!("{path}.hazard"),
        Some(&ValueType::Real),
    )?;
    require_type(&hazard_type, &ValueType::Real, &format!("{path}.hazard"))?;
    if matches!(&transition.hazard, Expr::Real { value } if *value < 0.0) {
        return Err(error(
            format!("{path}.hazard.value"),
            "literal hazard rate must be nonnegative",
        ));
    }

    for (index, effect) in transition.effects.iter().enumerate() {
        match effect {
            Effect::SetAttr { attr, value } => {
                let destination = find_attr(&table.attrs, attr).ok_or_else(|| {
                    error(
                        format!("{path}.effects[{index}].attr"),
                        format!("effect refers to unknown attribute '{attr}'"),
                    )
                })?;
                let expected = ValueType::from(&destination.ty);
                let actual = infer_expr(
                    value,
                    model,
                    model_box,
                    &table.attrs,
                    &format!("{path}.effects[{index}].value"),
                    Some(&expected),
                )?;
                require_type(
                    &actual,
                    &expected,
                    &format!("{path}.effects[{index}].value"),
                )?;
            }
        }
    }

    let mut claims = HashSet::new();
    for (index, claim) in transition.contests.iter().enumerate() {
        let claim_path = format!("{path}.contests[{index}]");
        let resource_type = infer_expr(
            &claim.resource,
            model,
            model_box,
            &table.attrs,
            &format!("{claim_path}.resource"),
            None,
        )?;
        if !matches!(resource_type, ValueType::Ref(_)) {
            return Err(error(
                format!("{claim_path}.resource"),
                format!(
                    "contested resource must be Ref-typed, found {}",
                    resource_type.name()
                ),
            ));
        }
        let identity = serde_json::to_string(&claim.resource).map_err(|source| {
            error(
                format!("{claim_path}.resource"),
                format!("could not identify resource claim: {source}"),
            )
        })?;
        if !claims.insert(identity) {
            return Err(error(
                format!("{claim_path}.resource"),
                "duplicate resource claim in transition",
            ));
        }
        if let ClaimOrdering::Key { expr } = &claim.ordering {
            let key_type = infer_expr(
                expr,
                model,
                model_box,
                &table.attrs,
                &format!("{claim_path}.ordering.expr"),
                None,
            )?;
            if !key_type.is_orderable() {
                return Err(error(
                    format!("{claim_path}.ordering.expr"),
                    format!("contest key must be orderable, found {}", key_type.name()),
                ));
            }
        }
    }

    for (index, effect) in transition.effects.iter().enumerate() {
        let Effect::SetAttr { attr, value } = effect;
        let destination = find_attr(&table.attrs, attr).ok_or_else(|| {
            error(
                format!("{path}.effects[{index}].attr"),
                format!("effect refers to unknown attribute '{attr}'"),
            )
        })?;
        if matches!(destination.ty, AttrType::Ref { .. })
            && !transition
                .contests
                .iter()
                .any(|claim| claim.resource.eq(value))
        {
            return Err(error(
                format!("{path}.effects[{index}].value"),
                format!("write to Ref attribute '{attr}' requires a matching resource claim"),
            ));
        }
    }

    Ok(())
}

fn validate_output(
    model: &Model,
    model_box: &Box,
    output: &OutputDecl,
    path: &str,
) -> Result<(), ValidationError> {
    match &output.builder {
        OutputBuilder::PerTable { table, fields } => {
            let source = find_table(model_box, table).ok_or_else(|| {
                error(
                    format!("{path}.builder.table"),
                    format!("output '{}' refers to unknown table '{table}'", output.name),
                )
            })?;
            if fields.len() != output.schema.len() {
                return Err(error(
                    format!("{path}.builder.fields"),
                    format!(
                        "output '{}' builder has {} fields but schema has {}",
                        output.name,
                        fields.len(),
                        output.schema.len()
                    ),
                ));
            }
            unique_names(
                fields.iter().map(|field| field.name.as_str()),
                &format!("{path}.builder.fields"),
                "output field",
            )?;
            for (index, (field, attr)) in fields.iter().zip(&output.schema).enumerate() {
                if field.name != attr.name {
                    return Err(error(
                        format!("{path}.builder.fields[{index}].name"),
                        format!(
                            "builder field '{}' does not match schema attribute '{}'",
                            field.name, attr.name
                        ),
                    ));
                }
                if let Some(filter) = &field.filter {
                    let filter_type = infer_expr(
                        filter,
                        model,
                        model_box,
                        &source.attrs,
                        &format!("{path}.builder.fields[{index}].filter"),
                        Some(&ValueType::Bool),
                    )?;
                    require_type(
                        &filter_type,
                        &ValueType::Bool,
                        &format!("{path}.builder.fields[{index}].filter"),
                    )?;
                }
                let field_type = infer_agg_op(
                    &field.op,
                    model,
                    model_box,
                    &source.attrs,
                    &format!("{path}.builder.fields[{index}].op"),
                )?;
                require_type(
                    &field_type,
                    &ValueType::from(&attr.ty),
                    &format!("{path}.builder.fields[{index}].op"),
                )?;
            }
        }
    }
    Ok(())
}

fn validate_wire(model: &Model, wire: &Wire, index: usize) -> Result<(), ValidationError> {
    let path = format!("$.wires[{index}]");
    let from_box = find_box(model, &wire.from.r#box).ok_or_else(|| {
        error(
            format!("{path}.from.box"),
            format!("wire refers to unknown source box '{}'", wire.from.r#box),
        )
    })?;
    let output = from_box
        .outputs
        .iter()
        .find(|output| output.name == wire.from.port)
        .ok_or_else(|| {
            error(
                format!("{path}.from.port"),
                format!(
                    "wire refers to unknown output '{}.{}'",
                    wire.from.r#box, wire.from.port
                ),
            )
        })?;
    let to_box = find_box(model, &wire.to.r#box).ok_or_else(|| {
        error(
            format!("{path}.to.box"),
            format!("wire refers to unknown destination box '{}'", wire.to.r#box),
        )
    })?;
    let input = to_box
        .inputs
        .iter()
        .find(|input| input.name == wire.to.port)
        .ok_or_else(|| {
            error(
                format!("{path}.to.port"),
                format!(
                    "wire refers to unknown input '{}.{}'",
                    wire.to.r#box, wire.to.port
                ),
            )
        })?;
    if output.schema != input.schema {
        return Err(error(
            path,
            format!(
                "wire schema mismatch between '{}.{}' and '{}.{}'",
                wire.from.r#box, wire.from.port, wire.to.r#box, wire.to.port
            ),
        ));
    }
    Ok(())
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

fn infer_expr(
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
            let lhs_type = infer_expr(
                lhs,
                model,
                model_box,
                row_attrs,
                &format!("{path}.lhs"),
                Some(&ValueType::Bool),
            )?;
            require_type(&lhs_type, &ValueType::Bool, &format!("{path}.lhs"))?;
            let rhs_type = infer_expr(
                rhs,
                model,
                model_box,
                row_attrs,
                &format!("{path}.rhs"),
                Some(&ValueType::Bool),
            )?;
            require_type(&rhs_type, &ValueType::Bool, &format!("{path}.rhs"))?;
            Ok(ValueType::Bool)
        }
        Expr::Not { expr } => {
            let actual = infer_expr(
                expr,
                model,
                model_box,
                row_attrs,
                &format!("{path}.expr"),
                Some(&ValueType::Bool),
            )?;
            require_type(&actual, &ValueType::Bool, &format!("{path}.expr"))?;
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
                let filter_type = infer_expr(
                    filter,
                    model,
                    model_box,
                    &input.schema,
                    &format!("{path}.agg.filter"),
                    Some(&ValueType::Bool),
                )?;
                require_type(
                    &filter_type,
                    &ValueType::Bool,
                    &format!("{path}.agg.filter"),
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
            let filter_type = infer_expr(
                filter,
                model,
                model_box,
                &target.attrs,
                &format!("{path}.filter"),
                Some(&ValueType::Bool),
            )?;
            require_type(&filter_type, &ValueType::Bool, &format!("{path}.filter"))?;
            infer_agg_op(op, model, model_box, &target.attrs, &format!("{path}.op"))
        }
    }
}

fn infer_agg_op(
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

fn require_type(
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

fn unique_names<'a>(
    names: impl IntoIterator<Item = &'a str>,
    path: &str,
    kind: &str,
) -> Result<(), ValidationError> {
    let mut seen = HashSet::new();
    for (index, name) in names.into_iter().enumerate() {
        if !seen.insert(name) {
            return Err(error(
                format!("{path}[{index}].name"),
                format!("duplicate {kind} name '{name}'"),
            ));
        }
    }
    Ok(())
}

fn find_box<'a>(model: &'a Model, name: &str) -> Option<&'a Box> {
    model.boxes.iter().find(|model_box| model_box.name == name)
}

fn find_table<'a>(model_box: &'a Box, name: &str) -> Option<&'a Table> {
    model_box.tables.iter().find(|table| table.name == name)
}

fn find_attr<'a>(attrs: &'a [Attr], name: &str) -> Option<&'a Attr> {
    attrs.iter().find(|attr| attr.name == name)
}

fn error(path: impl Into<String>, message: impl Into<String>) -> ValidationError {
    ValidationError::new(path, message)
}
