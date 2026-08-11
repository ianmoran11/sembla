//! Synthetic population and demographic-state tooling.

use super::*;

#[derive(Clone, Debug)]
pub(crate) struct SynthOptions {
    persons: usize,
    employers: usize,
    initial_infected: usize,
    seed: u64,
    out: String,
}

pub(crate) fn parse_synth_options(flags: &[String]) -> Result<SynthOptions, String> {
    let mut persons = None;
    let mut employers = None;
    let mut initial_infected = None;
    let mut seed = None;
    let mut out = None;
    let mut index = 0;
    while index < flags.len() {
        let flag = flags[index].as_str();
        let value = flags
            .get(index + 1)
            .ok_or_else(|| format!("missing value for '{flag}'"))?;
        match flag {
            "--persons" => set_once(&mut persons, parse_number(value, flag)?, flag)?,
            "--employers" => set_once(&mut employers, parse_number(value, flag)?, flag)?,
            "--initial-infected" => {
                set_once(&mut initial_infected, parse_number(value, flag)?, flag)?
            }
            "--seed" => set_once(&mut seed, parse_number(value, flag)?, flag)?,
            "--out" => set_once(&mut out, value.clone(), flag)?,
            _ => return Err(format!("unknown synth-pop flag '{flag}'")),
        }
        index += 2;
    }
    Ok(SynthOptions {
        persons: persons.ok_or_else(|| "missing required flag '--persons'".to_owned())?,
        employers: employers.ok_or_else(|| "missing required flag '--employers'".to_owned())?,
        initial_infected: initial_infected
            .ok_or_else(|| "missing required flag '--initial-infected'".to_owned())?,
        seed: seed.ok_or_else(|| "missing required flag '--seed'".to_owned())?,
        out: out.ok_or_else(|| "missing required flag '--out'".to_owned())?,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SynthStreams {
    birth: u64,
    overseas: u64,
    internal: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct SynthStateOptions {
    model: String,
    slots: usize,
    areas: usize,
    present_fraction: f64,
    streams: SynthStreams,
    seed: u64,
    out: String,
}

pub(crate) fn parse_synth_state_options(flags: &[String]) -> Result<SynthStateOptions, String> {
    let mut model = None;
    let mut slots = None;
    let mut areas = None;
    let mut present_fraction = None;
    let mut streams = None;
    let mut seed = None;
    let mut out = None;
    let mut index = 0;
    while index < flags.len() {
        let flag = flags[index].as_str();
        let value = flags
            .get(index + 1)
            .ok_or_else(|| format!("missing value for '{flag}'"))?;
        match flag {
            "--model" => set_once(&mut model, value.clone(), flag)?,
            "--slots" => set_once(&mut slots, parse_number(value, flag)?, flag)?,
            "--areas" => set_once(&mut areas, parse_number(value, flag)?, flag)?,
            "--present-fraction" => {
                let value: f64 = parse_number(value, flag)?;
                if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                    return Err(
                        "'--present-fraction' must be finite and between 0 and 1".to_owned()
                    );
                }
                set_once(&mut present_fraction, value, flag)?;
            }
            "--streams" => set_once(&mut streams, parse_synth_streams(value)?, flag)?,
            "--seed" => set_once(&mut seed, parse_number(value, flag)?, flag)?,
            "--out" => set_once(&mut out, value.clone(), flag)?,
            _ => return Err(format!("unknown synth-state flag '{flag}'")),
        }
        index += 2;
    }
    let slots = slots.ok_or_else(|| "missing required flag '--slots'".to_owned())?;
    let areas = areas.ok_or_else(|| "missing required flag '--areas'".to_owned())?;
    if slots == 0 {
        return Err("'--slots' must be greater than zero".to_owned());
    }
    if areas == 0 {
        return Err("'--areas' must be greater than zero".to_owned());
    }
    if slots > u32::MAX as usize || areas > u32::MAX as usize {
        return Err(
            "'--slots' and '--areas' must fit the state artifact Ref encoding (u32)".to_owned(),
        );
    }
    Ok(SynthStateOptions {
        model: model.ok_or_else(|| "missing required flag '--model'".to_owned())?,
        slots,
        areas,
        present_fraction: present_fraction
            .ok_or_else(|| "missing required flag '--present-fraction'".to_owned())?,
        streams: streams.ok_or_else(|| "missing required flag '--streams'".to_owned())?,
        seed: seed.ok_or_else(|| "missing required flag '--seed'".to_owned())?,
        out: out.ok_or_else(|| "missing required flag '--out'".to_owned())?,
    })
}

pub(crate) fn parse_synth_streams(value: &str) -> Result<SynthStreams, String> {
    let mut parsed = std::collections::BTreeMap::new();
    for item in value.split(',') {
        let (name, weight) = item.split_once(':').ok_or_else(|| {
            format!("invalid '--streams' value '{value}' (expected birth:B,overseas:O,internal:I)")
        })?;
        let weight: u64 = weight.parse().map_err(|_| {
            format!("invalid stream weight '{weight}' in '--streams' value '{value}'")
        })?;
        if !matches!(name, "birth" | "overseas" | "internal") {
            return Err(format!(
                "unknown stream '{name}' in '--streams' value '{value}'"
            ));
        }
        if parsed.insert(name, weight).is_some() {
            return Err(format!(
                "duplicate stream '{name}' in '--streams' value '{value}'"
            ));
        }
    }
    let streams = SynthStreams {
        birth: *parsed
            .get("birth")
            .ok_or_else(|| "'--streams' is missing 'birth'".to_owned())?,
        overseas: *parsed
            .get("overseas")
            .ok_or_else(|| "'--streams' is missing 'overseas'".to_owned())?,
        internal: *parsed
            .get("internal")
            .ok_or_else(|| "'--streams' is missing 'internal'".to_owned())?,
    };
    if streams
        .birth
        .checked_add(streams.overseas)
        .and_then(|sum| sum.checked_add(streams.internal))
        .filter(|sum| *sum > 0)
        .is_none()
    {
        return Err("'--streams' weights must have a nonzero, non-overflowing sum".to_owned());
    }
    if parsed.len() != 3 {
        return Err(format!(
            "invalid '--streams' value '{value}' (expected exactly birth:B,overseas:O,internal:I)"
        ));
    }
    Ok(streams)
}

pub(crate) fn synth_population(options: SynthOptions) -> i32 {
    let result = SyntheticPopulation::generate(
        options.persons,
        options.employers,
        options.initial_infected,
        options.seed,
    )
    .and_then(|population| population.write(&options.out));
    match result {
        Ok(()) => {
            println!(
                "persons={} employers={} initial_infected={} population={}",
                options.persons, options.employers, options.initial_infected, options.out
            );
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

pub(crate) fn synth_state(options: SynthStateOptions) -> i32 {
    match synth_state_result(&options) {
        Ok(companion) => {
            println!(
                "slots={} areas={} state={} companion_model={}",
                options.slots,
                options.areas,
                options.out,
                companion.display()
            );
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

pub(crate) fn synth_state_result(options: &SynthStateOptions) -> Result<PathBuf, String> {
    let (_, input) = read_input(&options.model)?;
    let mut model = match input {
        sembla_ir::ParsedInput::LegacyModel(model) => model,
        sembla_ir::ParsedInput::Plan(plan) => {
            sembla_ir::validate_plan(&plan)
                .map_err(|error| format!("{}: {error}", options.model))?;
            plan.model
        }
    };
    require_demographic_synth_schema(&model)?;
    for table in &mut model.boxes[0].tables {
        match table.name.as_str() {
            "area" => table.size_hint = options.areas as u64,
            "person_slot" | "slot_resource" => table.size_hint = options.slots as u64,
            _ => unreachable!("the demographic synthesis schema was checked"),
        }
    }
    let mut features = FeatureSet::new();
    if model
        .boxes
        .iter()
        .any(|model_box| !model_box.grouped_views.is_empty())
    {
        features.insert(GROUPED_OBSERVATIONS_FEATURE.to_owned());
    }
    let validated = sembla_ir::validate_with_features(model.clone(), &features)
        .map_err(|error| format!("{}: resized companion model: {error}", options.model))?;
    let tables = synthetic_demographic_tables(&validated, options)?;
    let companion = synth_state_companion_path(&options.out);
    for path in [Path::new(&options.out), companion.as_path()] {
        if path
            .try_exists()
            .map_err(|error| format!("{}: {error}", path.display()))?
        {
            return Err(format!(
                "refusing to overwrite synthesis output '{}'",
                path.display()
            ));
        }
    }
    let companion_json = sembla_ir::to_canonical_json(&model)
        .map_err(|error| format!("companion model serialization failed: {error}"))?;
    let mut companion_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&companion)
        .map_err(|error| format!("{}: {error}", companion.display()))?;
    if let Err(error) = std::io::Write::write_all(&mut companion_file, companion_json.as_bytes()) {
        drop(companion_file);
        let _ = std::fs::remove_file(&companion);
        return Err(format!("{}: {error}", companion.display()));
    }
    drop(companion_file);
    if let Err(error) = write_new_state_artifact(&options.out, &validated, &tables) {
        let _ = std::fs::remove_file(&companion);
        return Err(error.to_string());
    }
    Ok(companion)
}

pub(crate) fn synth_state_companion_path(out: &str) -> PathBuf {
    let mut path = std::ffi::OsString::from(out);
    path.push(".model.json");
    PathBuf::from(path)
}

pub(crate) fn require_demographic_synth_schema(model: &sembla_ir::Model) -> Result<(), String> {
    if model.boxes.len() != 1 || model.boxes[0].name != "demographic" {
        return Err(
            "synth-state benchmark tooling requires exactly one box named 'demographic'".to_owned(),
        );
    }
    let model_box = &model.boxes[0];
    if model_box.tables.len() != 3 {
        return Err(
            "synth-state benchmark tooling requires exactly area, person_slot, and slot_resource tables"
                .to_owned(),
        );
    }
    let area = synth_table(model_box, "area")?;
    let person = synth_table(model_box, "person_slot")?;
    let resource = synth_table(model_box, "slot_resource")?;
    require_synth_attrs(area, &[("area_key", AttrType::Int)])?;
    require_synth_attrs(resource, &[])?;
    require_synth_attrs(
        person,
        &[
            (
                "occupancy",
                AttrType::Enum {
                    variants: vec!["vacant".to_owned(), "present".to_owned()],
                },
            ),
            (
                "event",
                AttrType::Enum {
                    variants: [
                        "none_",
                        "birth",
                        "death",
                        "overseas_arrival",
                        "overseas_departure",
                        "internal_arrival",
                        "internal_departure",
                    ]
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
                },
            ),
            (
                "sex",
                AttrType::Enum {
                    variants: vec!["male".to_owned(), "female".to_owned()],
                },
            ),
            ("age_months", AttrType::Int),
            ("event_age_months", AttrType::Int),
            ("generation", AttrType::Int),
            (
                "entry_stream",
                AttrType::Enum {
                    variants: vec![
                        "birth_slot".to_owned(),
                        "overseas_slot".to_owned(),
                        "internal_slot".to_owned(),
                    ],
                },
            ),
            ("entry_age_months", AttrType::Int),
            (
                "area",
                AttrType::Ref {
                    table: "area".to_owned(),
                },
            ),
            (
                "slot_resource",
                AttrType::Ref {
                    table: "slot_resource".to_owned(),
                },
            ),
        ],
    )
}

pub(crate) fn synth_table<'a>(
    model_box: &'a sembla_ir::Box,
    name: &str,
) -> Result<&'a sembla_ir::Table, String> {
    model_box
        .tables
        .iter()
        .find(|table| table.name == name)
        .ok_or_else(|| format!("synth-state benchmark tooling requires table '{name}'"))
}

pub(crate) fn require_synth_attrs(
    table: &sembla_ir::Table,
    expected: &[(&str, AttrType)],
) -> Result<(), String> {
    if table.attrs.len() != expected.len() {
        return Err(format!(
            "synth-state table '{}' has {} attributes; expected the documented {} demographic column roles",
            table.name,
            table.attrs.len(),
            expected.len()
        ));
    }
    for (name, ty) in expected {
        let attr = table
            .attrs
            .iter()
            .find(|attr| attr.name == *name)
            .ok_or_else(|| {
                format!(
                    "synth-state table '{}' is missing documented demographic column role '{}'",
                    table.name, name
                )
            })?;
        if &attr.ty != ty {
            return Err(format!(
                "synth-state demographic column role '{}.{}' has type {:?}; expected {:?}",
                table.name, name, attr.ty, ty
            ));
        }
    }
    Ok(())
}

pub(crate) fn synthetic_demographic_tables(
    model: &sembla_ir::ValidatedModel,
    options: &SynthStateOptions,
) -> Result<Vec<TableInit>, String> {
    let present_count = ((options.slots as f64) * options.present_fraction).floor() as usize;
    let vacant_count = options.slots - present_count;
    let weight_sum = options.streams.birth as u128
        + options.streams.overseas as u128
        + options.streams.internal as u128;
    let birth_count =
        ((vacant_count as u128 * options.streams.birth as u128) / weight_sum) as usize;
    let overseas_count =
        ((vacant_count as u128 * options.streams.overseas as u128) / weight_sum) as usize;
    let model_box = &model.model().boxes[0];
    let mut tables = Vec::with_capacity(model_box.tables.len());
    for table in &model_box.tables {
        let row_count = usize::try_from(table.size_hint)
            .map_err(|_| format!("table '{}' row count is not representable", table.name))?;
        let mut columns = Vec::with_capacity(table.attrs.len());
        for attr in &table.attrs {
            let data = match (table.name.as_str(), attr.name.as_str()) {
                ("area", "area_key") => {
                    ColumnData::Int((0..row_count).map(|row| row as i64).collect())
                }
                ("person_slot", name) => synthetic_person_column(
                    name,
                    options,
                    present_count,
                    birth_count,
                    overseas_count,
                )?,
                _ => {
                    return Err(format!(
                        "synth-state has no documented mapping for '{}.{}'",
                        table.name, attr.name
                    ))
                }
            };
            columns.push(ColumnInit::new(&attr.name, data));
        }
        tables.push(TableInit::new(
            &model_box.name,
            &table.name,
            row_count,
            columns,
        ));
    }
    Ok(tables)
}

pub(crate) fn synthetic_person_column(
    name: &str,
    options: &SynthStateOptions,
    present_count: usize,
    birth_count: usize,
    overseas_count: usize,
) -> Result<ColumnData, String> {
    let rows = 0..options.slots;
    let data = match name {
        "occupancy" => ColumnData::Enum(
            rows.map(|row| u16::from(synth_slot_role(row, options, present_count).0))
                .collect(),
        ),
        "event" => ColumnData::Enum(vec![0; options.slots]),
        "sex" => ColumnData::Enum(
            rows.map(|row| (synth_word(options.seed, row, 1) & 1) as u16)
                .collect(),
        ),
        "age_months" => ColumnData::Int(
            rows.map(|row| {
                if synth_slot_role(row, options, present_count).0 {
                    (synth_word(options.seed, row, 2) % (90 * 12)) as i64
                } else {
                    0
                }
            })
            .collect(),
        ),
        "event_age_months" => ColumnData::Int(vec![-1; options.slots]),
        "generation" => ColumnData::Int(
            rows.map(|row| i64::from(synth_slot_role(row, options, present_count).0))
                .collect(),
        ),
        "entry_stream" => ColumnData::Enum(
            rows.map(|row| {
                let (_, vacant_rank) = synth_slot_role(row, options, present_count);
                if vacant_rank < birth_count {
                    0
                } else if vacant_rank < birth_count + overseas_count {
                    1
                } else {
                    2
                }
            })
            .collect(),
        ),
        "entry_age_months" => ColumnData::Int(
            rows.map(|row| {
                let (present, vacant_rank) = synth_slot_role(row, options, present_count);
                if present || vacant_rank < birth_count {
                    0
                } else if vacant_rank < birth_count + overseas_count {
                    (18 * 12 + synth_word(options.seed, row, 3) % (52 * 12)) as i64
                } else {
                    (synth_word(options.seed, row, 4) % (80 * 12)) as i64
                }
            })
            .collect(),
        ),
        "area" => ColumnData::Ref(
            rows.map(|row| (synth_word(options.seed, row, 5) % options.areas as u64) as u32)
                .collect(),
        ),
        "slot_resource" => ColumnData::Ref(rows.map(|row| row as u32).collect()),
        _ => {
            return Err(format!(
                "unknown synthesized demographic column role '{name}'"
            ))
        }
    };
    Ok(data)
}

pub(crate) fn synth_slot_role(
    row: usize,
    options: &SynthStateOptions,
    present_count: usize,
) -> (bool, usize) {
    let shift = options.seed % options.slots as u64;
    let coordinate = ((row as u64 + shift) % options.slots as u64) as usize;
    if coordinate < present_count {
        (true, 0)
    } else {
        (false, coordinate - present_count)
    }
}

pub(crate) fn synth_word(seed: u64, row: usize, lane: u64) -> u64 {
    let mut value = seed
        ^ (row as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ lane.wrapping_mul(0xd1b5_4a32_d192_ed03);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}
