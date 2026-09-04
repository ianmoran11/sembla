use crate::model::*;
use crate::ValidationError;
use std::collections::{BTreeMap, BTreeSet, HashSet};

mod expression;

use expression::*;

pub const GROUPED_OBSERVATIONS_FEATURE: &str = "grouped-observations";
pub const KNOWN_FEATURES: [&str; 1] = [GROUPED_OBSERVATIONS_FEATURE];
pub type FeatureSet = BTreeSet<String>;

/// A transition annotated with its dense ordinal and runtime identity word.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedTransition {
    pub box_index: usize,
    pub transition_index: usize,
    /// Index of the transition's source table within its box.
    pub table_index: usize,
    /// Dense declaration-order ordinal used for indexing and diagnostics.
    pub rule_id: u32,
    /// Philox coordinate word and deterministic conflict tie-break key.
    pub rule_word: u32,
}

/// Dense indices for one validated wire's endpoints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidatedWire {
    pub wire_index: usize,
    pub from_box_index: usize,
    pub output_index: usize,
    pub to_box_index: usize,
    pub input_index: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ResolvedModel {
    params: BTreeMap<String, usize>,
    boxes: BTreeMap<String, usize>,
    tables: Vec<BTreeMap<String, usize>>,
    inputs: Vec<BTreeMap<String, usize>>,
    outputs: Vec<BTreeMap<String, usize>>,
    transition_offsets: Vec<usize>,
    input_offsets: Vec<usize>,
    wires: Vec<ValidatedWire>,
}

impl ResolvedModel {
    fn new(model: &Model) -> Self {
        let params = named_indices(&model.params, |param| &param.name);
        let boxes = named_indices(&model.boxes, |model_box| &model_box.name);
        let tables: Vec<BTreeMap<String, usize>> = model
            .boxes
            .iter()
            .map(|model_box| named_indices(&model_box.tables, |table| &table.name))
            .collect();
        let inputs: Vec<BTreeMap<String, usize>> = model
            .boxes
            .iter()
            .map(|model_box| named_indices(&model_box.inputs, |input| &input.name))
            .collect();
        let outputs: Vec<BTreeMap<String, usize>> = model
            .boxes
            .iter()
            .map(|model_box| named_indices(&model_box.outputs, |output| &output.name))
            .collect();
        let mut transition_offsets = Vec::with_capacity(model.boxes.len() + 1);
        let mut offset = 0;
        for model_box in &model.boxes {
            transition_offsets.push(offset);
            offset += model_box.transitions.len();
        }
        transition_offsets.push(offset);
        let mut input_offsets = Vec::with_capacity(model.boxes.len() + 1);
        let mut input_offset = 0;
        for model_box in &model.boxes {
            input_offsets.push(input_offset);
            input_offset += model_box.inputs.len();
        }
        input_offsets.push(input_offset);
        let wires = model
            .wires
            .iter()
            .enumerate()
            .map(|(wire_index, wire)| {
                let from_box_index = boxes[&wire.from.r#box];
                let to_box_index = boxes[&wire.to.r#box];
                ValidatedWire {
                    wire_index,
                    from_box_index,
                    output_index: outputs[from_box_index][&wire.from.port],
                    to_box_index,
                    input_index: inputs[to_box_index][&wire.to.port],
                }
            })
            .collect();
        Self {
            params,
            boxes,
            tables,
            inputs,
            outputs,
            transition_offsets,
            input_offsets,
            wires,
        }
    }
}

fn named_indices<T>(items: &[T], name: impl Fn(&T) -> &String) -> BTreeMap<String, usize> {
    items
        .iter()
        .enumerate()
        .map(|(index, item)| (name(item).clone(), index))
        .collect()
}

/// A semantically valid model plus metadata derived during validation.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedModel {
    model: Model,
    transitions: Vec<ValidatedTransition>,
    resolved: ResolvedModel,
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

    pub fn transition(&self, rule_id: u32) -> Option<&ValidatedTransition> {
        self.transitions
            .get(rule_id as usize)
            .filter(|transition| transition.rule_id == rule_id)
    }

    pub fn transition_at(
        &self,
        box_index: usize,
        transition_index: usize,
    ) -> Option<&ValidatedTransition> {
        self.rule_id(box_index, transition_index)
            .and_then(|rule_id| self.transition(rule_id))
    }

    pub fn rule_id(&self, box_index: usize, transition_index: usize) -> Option<u32> {
        let start = *self.resolved.transition_offsets.get(box_index)?;
        let end = *self.resolved.transition_offsets.get(box_index + 1)?;
        (transition_index < end - start).then(|| self.transitions[start + transition_index].rule_id)
    }

    pub fn param_index(&self, name: &str) -> Option<usize> {
        self.resolved.params.get(name).copied()
    }

    pub fn box_index(&self, name: &str) -> Option<usize> {
        self.resolved.boxes.get(name).copied()
    }

    pub fn table_index(&self, box_index: usize, name: &str) -> Option<usize> {
        self.resolved.tables.get(box_index)?.get(name).copied()
    }

    pub fn input_index(&self, box_index: usize, name: &str) -> Option<usize> {
        self.resolved.inputs.get(box_index)?.get(name).copied()
    }

    pub fn output_index(&self, box_index: usize, name: &str) -> Option<usize> {
        self.resolved.outputs.get(box_index)?.get(name).copied()
    }

    pub fn global_input_index(&self, box_index: usize, input_index: usize) -> Option<usize> {
        let start = *self.resolved.input_offsets.get(box_index)?;
        let end = *self.resolved.input_offsets.get(box_index + 1)?;
        (input_index < end - start).then_some(start + input_index)
    }

    pub fn wires(&self) -> &[ValidatedWire] {
        &self.resolved.wires
    }

    pub(crate) fn with_rule_words(mut self, words: &[u32]) -> Self {
        assert_eq!(
            self.transitions.len(),
            words.len(),
            "validated rule-word overlay must cover every dense transition"
        );
        for (transition, &word) in self.transitions.iter_mut().zip(words) {
            transition.rule_word = word;
        }
        self
    }
}

/// Validates all references and expression types with no provisional runtime features.
pub fn validate(model: Model) -> Result<ValidatedModel, ValidationError> {
    validate_with_features(model, &FeatureSet::new())
}

/// Validates a model under an explicit, per-execution feature set.
///
/// The set is data threaded by callers, never process-global state or a Cargo feature.
pub fn validate_with_features(
    model: Model,
    enabled_features: &FeatureSet,
) -> Result<ValidatedModel, ValidationError> {
    validate_model(&model, enabled_features)?;
    let resolved = ResolvedModel::new(&model);

    let mut transitions = Vec::new();
    for (box_index, model_box) in model.boxes.iter().enumerate() {
        for transition_index in 0..model_box.transitions.len() {
            let rule_id = u32::try_from(transitions.len()).map_err(|_| {
                ValidationError::new(
                    format!("$.boxes[{box_index}].transitions[{transition_index}]"),
                    "too many transitions to assign a u32 rule_id",
                )
            })?;
            if rule_id >= u32::MAX - 1 {
                return Err(ValidationError::new(
                    format!("$.boxes[{box_index}].transitions[{transition_index}]"),
                    "too many transitions: rule_id namespaces u32::MAX - 1 and u32::MAX are reserved",
                ));
            }
            transitions.push(ValidatedTransition {
                box_index,
                transition_index,
                table_index: resolved.tables[box_index]
                    [&model_box.transitions[transition_index].table],
                rule_id,
                // Legacy models retain the exact positional RNG/tie-break identity.
                rule_word: rule_id,
            });
        }
    }

    Ok(ValidatedModel {
        model,
        transitions,
        resolved,
    })
}

fn validate_model(model: &Model, enabled_features: &FeatureSet) -> Result<(), ValidationError> {
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
    unique_names(
        model.summaries.iter().map(|summary| summary.name.as_str()),
        "$.summaries",
        "summary",
    )?;

    for (index, param) in model.params.iter().enumerate() {
        validate_param(param, index)?;
    }
    for (index, model_box) in model.boxes.iter().enumerate() {
        validate_box(model, model_box, index, enabled_features)?;
    }
    for (index, summary) in model.summaries.iter().enumerate() {
        validate_summary(model, summary, index)?;
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
        if param.ty == ParamType::Int {
            return Err(error(
                format!("{base}.prior"),
                format!("integer parameter '{}' cannot declare a prior", param.name),
            ));
        }
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

fn validate_box(
    model: &Model,
    model_box: &Box,
    box_index: usize,
    enabled_features: &FeatureSet,
) -> Result<(), ValidationError> {
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
    unique_names(
        model_box.views.iter().map(|view| view.name.as_str()),
        &format!("{base}.views"),
        "view",
    )?;
    let mut view_names = model_box
        .views
        .iter()
        .map(|view| view.name.as_str())
        .collect::<HashSet<_>>();
    for (index, view) in model_box.grouped_views.iter().enumerate() {
        if !view_names.insert(view.name.as_str()) {
            return Err(error(
                format!("{base}.grouped_views[{index}].name"),
                format!("duplicate view name '{}'", view.name),
            ));
        }
        if !enabled_features.contains(GROUPED_OBSERVATIONS_FEATURE) {
            return Err(error(
                format!("{base}.grouped_views[{index}]"),
                format!(
                    "grouped view '{}' requires --enable {GROUPED_OBSERVATIONS_FEATURE}",
                    view.name
                ),
            ));
        }
    }

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
    for (view_index, view) in model_box.views.iter().enumerate() {
        validate_view(
            model,
            model_box,
            view,
            &format!("{base}.views[{view_index}]"),
        )?;
    }
    for (view_index, view) in model_box.grouped_views.iter().enumerate() {
        validate_grouped_view(
            model,
            model_box,
            view,
            &format!("{base}.grouped_views[{view_index}]"),
        )?;
    }

    Ok(())
}

fn validate_grouped_view(
    model: &Model,
    model_box: &Box,
    view: &GroupedViewDecl,
    path: &str,
) -> Result<(), ValidationError> {
    let table = find_table(model_box, &view.table).ok_or_else(|| {
        error(
            format!("{path}.table"),
            format!(
                "grouped view '{}' refers to unknown table '{}'",
                view.name, view.table
            ),
        )
    })?;
    if !(1..=4).contains(&view.keys.len()) {
        return Err(error(
            format!("{path}.keys"),
            format!(
                "grouped view '{}' requires between 1 and 4 keys, found {}",
                view.name,
                view.keys.len()
            ),
        ));
    }
    for (index, key) in view.keys.iter().enumerate() {
        let key_path = format!("{path}.keys[{index}]");
        let attr = find_attr(&table.attrs, &key.attr).ok_or_else(|| {
            error(
                format!("{key_path}.attr"),
                format!(
                    "grouped view key refers to unknown attribute '{}'",
                    key.attr
                ),
            )
        })?;
        match (&attr.ty, key.band_width) {
            (AttrType::Int, Some(width)) if width >= 1 => {}
            (AttrType::Int, Some(_)) => {
                return Err(error(
                    format!("{key_path}.band_width"),
                    "grouped Int key band_width must be at least 1",
                ));
            }
            (AttrType::Int, None) => {
                return Err(error(
                    format!("{key_path}.band_width"),
                    format!("grouped Int key '{}' requires band_width", key.attr),
                ));
            }
            (AttrType::Enum { .. } | AttrType::Ref { .. }, None) => {}
            (AttrType::Enum { .. } | AttrType::Ref { .. }, Some(_)) => {
                return Err(error(
                    format!("{key_path}.band_width"),
                    format!(
                        "grouped non-Int key '{}' must not declare band_width",
                        key.attr
                    ),
                ));
            }
            (AttrType::Real, _) => {
                return Err(error(
                    format!("{key_path}.attr"),
                    format!(
                        "grouped key '{}' must have type Enum, Ref, or Int",
                        key.attr
                    ),
                ));
            }
        }
    }
    if let Some(filter) = &view.filter {
        validate_grouped_filter_expr(filter, &format!("{path}.filter"))?;
        require_expr_type(
            filter,
            model,
            model_box,
            &table.attrs,
            &format!("{path}.filter"),
            &ValueType::Bool,
        )?;
    }
    Ok(())
}

fn validate_grouped_filter_expr(expr: &Expr, path: &str) -> Result<(), ValidationError> {
    match expr {
        Expr::Input { .. } | Expr::Agg { .. } => Err(error(
            path,
            "aggregates are not supported in grouped view filters",
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
            validate_grouped_filter_expr(lhs, &format!("{path}.lhs"))?;
            validate_grouped_filter_expr(rhs, &format!("{path}.rhs"))
        }
        Expr::Not { expr } => validate_grouped_filter_expr(expr, &format!("{path}.expr")),
        Expr::Real { .. }
        | Expr::Int { .. }
        | Expr::Bool { .. }
        | Expr::Enum { .. }
        | Expr::Param { .. }
        | Expr::SelfAttr { .. }
        | Expr::EnumIs { .. } => Ok(()),
    }
}

fn validate_view(
    model: &Model,
    model_box: &Box,
    view: &ViewDecl,
    path: &str,
) -> Result<(), ValidationError> {
    let table = find_table(model_box, &view.table).ok_or_else(|| {
        error(
            format!("{path}.table"),
            format!(
                "view '{}' refers to unknown table '{}'",
                view.name, view.table
            ),
        )
    })?;

    if let Some(filter) = &view.filter {
        require_expr_type(
            filter,
            model,
            model_box,
            &table.attrs,
            &format!("{path}.filter"),
            &ValueType::Bool,
        )?;
    }

    match (&view.reduce, &view.value) {
        (ViewReduce::Count, Some(_)) => {
            return Err(error(
                format!("{path}.value"),
                format!("count view '{}' must not declare a value", view.name),
            ));
        }
        (ViewReduce::Count, None) => {}
        (_, None) => {
            return Err(error(
                format!("{path}.value"),
                format!(
                    "{:?} view '{}' must declare a value",
                    view.reduce, view.name
                ),
            ));
        }
        (_, Some(value)) => {
            let value_type = infer_expr(
                value,
                model,
                model_box,
                &table.attrs,
                &format!("{path}.value"),
                None,
            )?;
            if !value_type.is_numeric() {
                return Err(error(
                    format!("{path}.value"),
                    format!("view value must be numeric, found {}", value_type.name()),
                ));
            }
        }
    }

    Ok(())
}

fn validate_summary(
    model: &Model,
    summary: &SummaryDecl,
    index: usize,
) -> Result<(), ValidationError> {
    let base = format!("$.summaries[{index}]");
    let model_box = find_box(model, &summary.r#box).ok_or_else(|| {
        error(
            format!("{base}.box"),
            format!(
                "summary '{}' refers to unknown box '{}'",
                summary.name, summary.r#box
            ),
        )
    })?;
    if find_view(model_box, &summary.view).is_none() {
        return Err(error(
            format!("{base}.view"),
            format!(
                "summary '{}' refers to unknown view '{}.{}'",
                summary.name, summary.r#box, summary.view
            ),
        ));
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

    require_expr_type(
        &transition.guard,
        model,
        model_box,
        &table.attrs,
        &format!("{path}.guard"),
        &ValueType::Bool,
    )?;

    require_expr_type(
        &transition.hazard,
        model,
        model_box,
        &table.attrs,
        &format!("{path}.hazard"),
        &ValueType::Real,
    )?;
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
                require_expr_type(
                    value,
                    model,
                    model_box,
                    &table.attrs,
                    &format!("{path}.effects[{index}].value"),
                    &expected,
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
                    require_expr_type(
                        filter,
                        model,
                        model_box,
                        &source.attrs,
                        &format!("{path}.builder.fields[{index}].filter"),
                        &ValueType::Bool,
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

fn find_view<'a>(model_box: &'a Box, name: &str) -> Option<&'a ViewDecl> {
    model_box.views.iter().find(|view| view.name == name)
}

fn find_attr<'a>(attrs: &'a [Attr], name: &str) -> Option<&'a Attr> {
    attrs.iter().find(|attr| attr.name == name)
}

fn error(path: impl Into<String>, message: impl Into<String>) -> ValidationError {
    ValidationError::new(path, message)
}
