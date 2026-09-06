//! Shared CLI input, parameter, population, and identity plumbing.

use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum BackendSelection {
    #[default]
    Cpu,
    Cuda,
}

pub(crate) fn parse_backend(value: &str) -> Result<BackendSelection, String> {
    match value {
        "cpu" => Ok(BackendSelection::Cpu),
        "cuda" => Ok(BackendSelection::Cuda),
        _ => Err(format!(
            "invalid backend '{value}' (expected 'cpu' or 'cuda')"
        )),
    }
}

pub(crate) fn parse_feature(value: &str) -> Result<String, String> {
    match value {
        GROUPED_OBSERVATIONS_FEATURE => Ok(value.to_owned()),
        _ => Err(format!(
            "unknown feature '{value}' (known features: {GROUPED_OBSERVATIONS_FEATURE})"
        )),
    }
}

pub(crate) fn parse_number<T: std::str::FromStr>(value: &str, flag: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("invalid numeric value '{value}' for '{flag}'"))
}

pub(crate) fn set_once<T>(slot: &mut Option<T>, value: T, flag: &str) -> Result<(), String> {
    if slot.is_some() {
        Err(format!("duplicate flag '{flag}'"))
    } else {
        *slot = Some(value);
        Ok(())
    }
}

pub(crate) fn read_input(path: &str) -> Result<(String, sembla_ir::ParsedInput), String> {
    let source = std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    let input = sembla_ir::parse_input(&source).map_err(|error| format!("{path}: {error}"))?;
    Ok((source, input))
}

pub(crate) fn read_model(path: &str) -> Result<sembla_ir::Model, String> {
    match read_input(path)?.1 {
        sembla_ir::ParsedInput::LegacyModel(model) => Ok(model),
        sembla_ir::ParsedInput::Plan(_) => Err(format!("{path}: {PLAN_NOT_RUNNABLE}")),
    }
}

pub(crate) fn read_validated(path: &str) -> Result<sembla_ir::ValidatedModel, String> {
    sembla_ir::validate(read_model(path)?).map_err(|error| format!("{path}: {error}"))
}

pub(crate) struct RunInput {
    pub(crate) model: sembla_ir::ValidatedModel,
    pub(crate) plan: Option<sembla_ir::ExecutablePlanV1>,
}

pub(crate) fn read_run_input(path: &str, options: &RunOptions) -> Result<RunInput, String> {
    read_executable_input(path, options.dt, &options.enabled_features)
}

pub(crate) fn read_executable_input(
    path: &str,
    dt: Option<f64>,
    enabled_features: &FeatureSet,
) -> Result<RunInput, String> {
    let (source, input) = read_input(path)?;
    match input {
        sembla_ir::ParsedInput::LegacyModel(mut model) => {
            if let Some(dt) = dt {
                model.dt = dt;
            }
            let model = sembla_ir::validate_with_features(model, enabled_features)
                .map_err(|error| format!("{path}: {error}"))?;
            Ok(RunInput { model, plan: None })
        }
        sembla_ir::ParsedInput::Plan(plan) => {
            let validated =
                sembla_ir::validate_plan(&plan).map_err(|error| format!("{path}: {error}"))?;
            // Plan validation accepts an artifact that accurately describes its
            // features. Executing it separately requires the runtime flag.
            sembla_ir::validate_with_features(plan.model.clone(), enabled_features)
                .map_err(|error| format!("{path}: {error}"))?;
            require_canonical_plan(path, &source)?;
            if dt.is_some() {
                return Err(format!(
                    "{path}: plan envelopes do not support --dt overrides; edit and re-canonicalize the plan instead"
                ));
            }
            Ok(RunInput {
                model: validated.model_with_rule_words(),
                plan: Some(plan),
            })
        }
    }
}

pub(crate) fn require_canonical_plan(path: &str, source: &str) -> Result<(), String> {
    let value: serde_json::Value = serde_json::from_str(source)
        .map_err(|error| format!("{path}: invalid JSON after plan parsing: {error}"))?;
    let canonical = sembla_ir::to_canonical_string(&value)
        .map_err(|error| format!("{path}: canonical serialization failed: {error}"))?;
    if canonical == source {
        Ok(())
    } else {
        Err(format!("{path}: plan file is not canonical"))
    }
}

pub(crate) fn read_validated_plan(path: &str) -> Result<sembla_ir::ExecutablePlanV1, String> {
    let (source, input) = read_input(path)?;
    let sembla_ir::ParsedInput::Plan(plan) = input else {
        return Err(format!("{path}: expected an executable plan envelope"));
    };
    sembla_ir::validate_plan(&plan).map_err(|error| format!("{path}: {error}"))?;
    require_canonical_plan(path, &source)?;
    Ok(plan)
}

pub(crate) fn resolve_params(
    model: &sembla_ir::ValidatedModel,
    path: Option<&str>,
) -> Result<ParamEnv, String> {
    let Some(path) = path else {
        return Ok(ParamEnv::defaults(model));
    };
    let overrides = read_param_overrides(model, path)?;
    ParamEnv::resolve(model, &overrides).map_err(|error| format!("{path}: {error}"))
}

pub(crate) fn param_value_from_json(
    declaration: &sembla_ir::ParamDecl,
    value: &serde_json::Value,
    context: &str,
) -> Result<ParamValue, String> {
    match declaration.ty {
        ParamType::Real => Ok(ParamValue::Real {
            value: value.as_f64().ok_or_else(|| {
                format!(
                    "{context}: parameter '{}' must have type real",
                    declaration.name
                )
            })?,
        }),
        ParamType::Int => Ok(ParamValue::Int {
            value: value.as_i64().ok_or_else(|| {
                format!(
                    "{context}: parameter '{}' must have type int",
                    declaration.name
                )
            })?,
        }),
    }
}

pub(crate) fn read_param_overrides(
    model: &sembla_ir::ValidatedModel,
    path: &str,
) -> Result<Vec<ParamOverride>, String> {
    let source = std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    let value: serde_json::Value =
        serde_json::from_str(&source).map_err(|error| format!("{path}: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| format!("{path}: parameter overrides must be a JSON object"))?;
    let mut overrides = Vec::with_capacity(object.len());
    for (name, value) in object {
        let declaration = model
            .model()
            .params
            .iter()
            .find(|parameter| parameter.name == *name)
            .ok_or_else(|| format!("{path}: unknown parameter '{name}'"))?;
        let value = param_value_from_json(declaration, value, path)?;
        overrides.push(ParamOverride::new(name, value));
    }
    ParamEnv::resolve(model, &overrides).map_err(|error| format!("{path}: {error}"))?;
    Ok(overrides)
}

pub(crate) type StateHash = [u8; 32];
pub(crate) type PerTickHashes = Option<Vec<StateHash>>;
pub(crate) type ComparedPerTickHashes<'a> = (&'a [StateHash], &'a [StateHash]);

pub(crate) fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

pub(crate) fn repository_commit() -> Result<String, String> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|error| format!("could not resolve repository commit for timing JSON: {error}"))?;
    if !output.status.success() {
        return Err("could not resolve repository commit for timing JSON".to_owned());
    }
    let commit = String::from_utf8(output.stdout)
        .map_err(|_| "repository commit is not UTF-8".to_owned())?
        .trim()
        .to_owned();
    if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("repository commit is not a full SHA-1".to_owned());
    }
    Ok(commit)
}

pub(crate) fn current_binary_sha256() -> Result<String, String> {
    let binary = std::env::current_exe()
        .map_err(|error| format!("could not locate current binary for timing JSON: {error}"))?;
    let bytes = std::fs::read(&binary).map_err(|error| format!("{}: {error}", binary.display()))?;
    Ok(hex(&Sha256::digest(bytes)))
}

pub(crate) fn initializers_from_population(
    model: &sembla_ir::ValidatedModel,
    population: &SyntheticPopulation,
) -> Result<Vec<TableInit>, String> {
    let population_boxes = model
        .model()
        .boxes
        .iter()
        .filter(|model_box| {
            model_box.tables.iter().any(|table| {
                table.name == "person"
                    && table.attrs.iter().any(|attr| {
                        attr.name == "health"
                            && matches!(&attr.ty, AttrType::Enum { variants } if variants == &["S", "I", "R"])
                    })
                    && table.attrs.iter().any(|attr| {
                        attr.name == "employer"
                            && matches!(&attr.ty, AttrType::Ref { table } if table == "employer")
                    })
            }) && model_box.tables.iter().any(|table| table.name == "employer")
        })
        .collect::<Vec<_>>();
    if population_boxes.is_empty() {
        return Err(
            "population file requires exactly one compatible person/employer schema, found 0"
                .to_owned(),
        );
    }
    let controller_boxes = model
        .model()
        .boxes
        .iter()
        .filter(|model_box| {
            model_box.tables.iter().any(|table| {
                table.name == "controller"
                    && table.size_hint == 1
                    && table.attrs.iter().any(|attr| attr.name == "mode")
                    && table.attrs.iter().any(|attr| attr.name == "modifier")
            })
        })
        .collect::<Vec<_>>();

    let mut initial = Vec::new();
    let mut initialized_population_boxes = Vec::new();
    let mut initialized_controller_boxes = Vec::new();
    if let [population_box] = population_boxes.as_slice() {
        // Preserve the pre-PRD single-population behavior exactly.
        let controller_box = match controller_boxes.as_slice() {
            [] => None,
            [model_box] => Some(*model_box),
            _ => {
                return Err(format!(
                    "population file found {} compatible controller schemas",
                    controller_boxes.len()
                ));
            }
        };
        initial.extend(match controller_box {
            Some(controller) => population
                .sir_policy_table_initializers_for_boxes(&population_box.name, &controller.name),
            None => population.sir_table_initializers_for_box(&population_box.name),
        });
        initialized_population_boxes.push(population_box.name.as_str());
        if let Some(controller) = controller_box {
            initialized_controller_boxes.push(controller.name.as_str());
        }
    } else {
        for (index, population_box) in population_boxes.iter().enumerate() {
            let scope = parent_occurrence_path(&population_box.name);
            if population_boxes[..index]
                .iter()
                .any(|other| parent_occurrence_path(&other.name) == scope)
            {
                return Err(format!(
                    "population file found multiple compatible population schemas in occurrence scope '{scope}'"
                ));
            }
            let matching_controllers = controller_boxes
                .iter()
                .filter(|controller| parent_occurrence_path(&controller.name) == scope)
                .copied()
                .collect::<Vec<_>>();
            let controller = match matching_controllers.as_slice() {
                [] if controller_boxes.is_empty() => None,
                [controller] => Some(*controller),
                _ => {
                    return Err(format!(
                        "population file found {} compatible controller schemas in occurrence scope '{scope}'",
                        matching_controllers.len()
                    ));
                }
            };
            initial.extend(match controller {
                Some(controller) => population.sir_policy_table_initializers_for_boxes(
                    &population_box.name,
                    &controller.name,
                ),
                None => population.sir_table_initializers_for_box(&population_box.name),
            });
            initialized_population_boxes.push(population_box.name.as_str());
            if let Some(controller) = controller {
                initialized_controller_boxes.push(controller.name.as_str());
            }
        }
        if let Some(unmatched) = controller_boxes
            .iter()
            .find(|controller| !initialized_controller_boxes.contains(&controller.name.as_str()))
        {
            return Err(format!(
                "population file found unmatched compatible controller schema '{}'",
                unmatched.name
            ));
        }
    }

    for model_box in &model.model().boxes {
        for table in &model_box.tables {
            if (initialized_population_boxes.contains(&model_box.name.as_str())
                && (table.name == "person" || table.name == "employer"))
                || (initialized_controller_boxes.contains(&model_box.name.as_str())
                    && table.name == "controller")
            {
                continue;
            }
            let row_count = usize::try_from(table.size_hint).map_err(|_| {
                format!("{}.{} size_hint exceeds usize", model_box.name, table.name)
            })?;
            let columns = table
                .attrs
                .iter()
                .map(|attr| {
                    let data = match &attr.ty {
                        AttrType::Real => ColumnData::Real(vec![0.0; row_count]),
                        AttrType::Int => ColumnData::Int(vec![0; row_count]),
                        AttrType::Enum { .. } => ColumnData::Enum(vec![0; row_count]),
                        AttrType::Ref { .. } => ColumnData::Ref(vec![0; row_count]),
                    };
                    ColumnInit::new(&attr.name, data)
                })
                .collect();
            initial.push(TableInit::new(
                &model_box.name,
                &table.name,
                row_count,
                columns,
            ));
        }
    }
    Ok(initial)
}

pub(crate) struct InitializedTables {
    pub(crate) tables: Vec<TableInit>,
    pub(crate) state_hash: Option<sembla_ir::HashRecordV1>,
}

pub(crate) fn state_artifact_tuple(hash: sembla_ir::HashRecordV1) -> manifest::StateArtifactTuple {
    manifest::StateArtifactTuple {
        format: STATE_ARTIFACT_SCHEMA.to_owned(),
        hash,
    }
}

pub(crate) fn initialized_tables(
    model: &sembla_ir::ValidatedModel,
    population_spec: &str,
) -> Result<InitializedTables, String> {
    if let Ok(population) = population_spec.parse::<usize>() {
        return Ok(InitializedTables {
            tables: initialize_population(model, population),
            state_hash: None,
        });
    }
    let bytes =
        std::fs::read(population_spec).map_err(|error| format!("{population_spec}: {error}"))?;
    initialized_tables_from_bytes(model, population_spec, &bytes)
}

pub(crate) fn initialized_tables_from_population(
    model: &sembla_ir::ValidatedModel,
    population_spec: &str,
    bytes: Option<Vec<u8>>,
) -> Result<InitializedTables, String> {
    match bytes {
        Some(bytes) => initialized_tables_from_bytes(model, population_spec, &bytes),
        None => initialized_tables(model, population_spec),
    }
}

fn initialized_tables_from_bytes(
    model: &sembla_ir::ValidatedModel,
    population_spec: &str,
    bytes: &[u8],
) -> Result<InitializedTables, String> {
    use sembla_runtime::state_artifact::{
        into_table_inits, read_bytes_with_hash, sniff_magic_bytes,
    };
    match sniff_magic_bytes(bytes) {
        StateKind::SemblaPop => Ok(InitializedTables {
            tables: initializers_from_population(
                model,
                &SyntheticPopulation::decode(bytes).map_err(|error| format!("{population_spec}: {error}"))?,
            )?,
            state_hash: None,
        }),
        StateKind::SemblaState => {
            let (artifact, state_hash) = read_bytes_with_hash(bytes).map_err(|error| error.to_string())?;
            let tables = into_table_inits(artifact, model).map_err(|error| error.to_string())?;
            Ok(InitializedTables {
                tables,
                state_hash: Some(state_hash),
            })
        }
        StateKind::Unknown => Err(format!(
            "unrecognized population artifact magic in '{population_spec}'; supported formats: SEMBLA_POP, SEMBLA_STATE"
        )),
    }
}

pub(crate) fn params_from_manifest(
    model: &sembla_ir::ValidatedModel,
    values: &std::collections::BTreeMap<String, manifest::ResolvedValue>,
) -> Result<ParamEnv, String> {
    let expected_names = model
        .model()
        .params
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let actual_names = values
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    if actual_names != expected_names {
        return Err(format!(
            "manifest resolved_theta parameter names mismatch: recorded={actual_names:?} expected={expected_names:?}"
        ));
    }
    let overrides = values
        .iter()
        .map(|(name, value)| {
            let value = match value {
                manifest::ResolvedValue::Real(value) => ParamValue::Real { value: *value },
                manifest::ResolvedValue::Int(value) => ParamValue::Int { value: *value },
            };
            ParamOverride::new(name, value)
        })
        .collect::<Vec<_>>();
    ParamEnv::resolve(model, &overrides)
        .map_err(|error| format!("manifest resolved_theta: {error}"))
}

pub(crate) fn canonical_params(params: &ParamEnv) -> Result<String, String> {
    let mut object = String::from("{");
    for (index, (name, value)) in params.values().enumerate() {
        if index != 0 {
            object.push(',');
        }
        object.push_str(&serde_json::to_string(name).map_err(|error| error.to_string())?);
        object.push(':');
        match value {
            ParamValue::Real { value } => {
                object.push_str(&serde_json::to_string(value).map_err(|error| error.to_string())?)
            }
            ParamValue::Int { value } => object.push_str(&value.to_string()),
        }
    }
    object.push('}');
    Ok(object)
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

pub(crate) fn initialize_population(
    model: &sembla_ir::ValidatedModel,
    population: usize,
) -> Vec<TableInit> {
    let mut initial = Vec::new();
    let composed = model.model().boxes.len() > 1 || !model.model().wires.is_empty();
    for model_box in &model.model().boxes {
        for table in &model_box.tables {
            let row_count = if composed && table.size_hint != 0 {
                usize::try_from(table.size_hint).expect("table size_hint exceeds usize")
            } else {
                population
            };
            let columns = table
                .attrs
                .iter()
                .map(|attr| {
                    let data = match attr.ty {
                        AttrType::Real => ColumnData::Real(vec![0.0; row_count]),
                        AttrType::Int => ColumnData::Int(vec![0; row_count]),
                        AttrType::Enum { .. } => ColumnData::Enum(vec![0; row_count]),
                        AttrType::Ref { .. } => ColumnData::Ref(vec![0; row_count]),
                    };
                    ColumnInit::new(&attr.name, data)
                })
                .collect();
            initial.push(TableInit::new(
                &model_box.name,
                &table.name,
                row_count,
                columns,
            ));
        }
    }
    initial
}
