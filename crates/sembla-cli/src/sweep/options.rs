//! Sweep option and theta-input handling.

use super::*;

#[derive(Clone, Debug)]
pub(crate) struct SweepOptions {
    pub(crate) seed: u64,
    pub(crate) draws: Option<u32>,
    pub(crate) theta_file: Option<String>,
    pub(crate) noise_mode: manifest::NoiseMode,
    pub(crate) ticks: u32,
    pub(crate) population: String,
    pub(crate) out: String,
    pub(crate) params: Option<String>,
    pub(crate) export_pairs: Option<String>,
    pub(crate) timing_json: Option<String>,
    pub(crate) backend: BackendSelection,
    /// `None` preserves whether the supported option was omitted. The
    /// resolved execution default is still exactly one worker.
    pub(crate) draw_workers: Option<usize>,
    pub(crate) enabled_features: FeatureSet,
}

pub(crate) fn parse_sweep_options(flags: &[String]) -> Result<SweepOptions, String> {
    let mut seed = None;
    let mut draws = None;
    let mut ticks = None;
    let mut population = None;
    let mut out = None;
    let mut params = None;
    let mut theta_file = None;
    let mut noise_mode = None;
    let mut export_pairs = None;
    let mut timing_json = None;
    let mut backend = None;
    let mut draw_workers = None;
    let mut enabled_features = FeatureSet::new();
    let mut index = 0;
    while index < flags.len() {
        let flag = flags[index].as_str();
        let value = flags
            .get(index + 1)
            .ok_or_else(|| format!("missing value for '{flag}'"))?;
        match flag {
            "--seed" => set_once(&mut seed, parse_number(value, flag)?, flag)?,
            "--draws" => set_once(&mut draws, parse_number(value, flag)?, flag)?,
            "--theta-file" => set_once(&mut theta_file, value.clone(), flag)?,
            "--noise" => {
                let value = match value.as_str() {
                    "crn" => manifest::NoiseMode::Crn,
                    "independent" => manifest::NoiseMode::Independent,
                    _ => {
                        return Err(format!(
                            "invalid value '{value}' for '--noise' (expected 'crn' or 'independent')"
                        ));
                    }
                };
                set_once(&mut noise_mode, value, flag)?;
            }
            "--ticks" => set_once(&mut ticks, parse_number(value, flag)?, flag)?,
            "--population" => {
                if value.parse::<usize>().is_err() && !Path::new(value).is_file() {
                    return Err(format!(
                        "invalid numeric value or population file '{value}' for '{flag}'"
                    ));
                }
                set_once(&mut population, value.clone(), flag)?;
            }
            "--out" => set_once(&mut out, value.clone(), flag)?,
            "--params" => set_once(&mut params, value.clone(), flag)?,
            "--export-pairs" => set_once(&mut export_pairs, value.clone(), flag)?,
            "--timing-json" => set_once(&mut timing_json, value.clone(), flag)?,
            "--backend" => set_once(&mut backend, parse_backend(value)?, flag)?,
            "--draw-workers" => {
                let value: usize = parse_number(value, flag)?;
                if value == 0 {
                    return Err("'--draw-workers' must be greater than zero".to_owned());
                }
                set_once(&mut draw_workers, value, flag)?;
            }
            "--enable" => {
                enabled_features.insert(parse_feature(value)?);
            }
            _ => return Err(format!("unknown sweep flag '{flag}'")),
        }
        index += 2;
    }
    if draws.is_some() && theta_file.is_some() {
        return Err("'--theta-file' cannot be combined with '--draws'".to_owned());
    }
    if draws.is_none() && theta_file.is_none() {
        return Err("missing required flag '--draws' or '--theta-file'".to_owned());
    }
    if draws == Some(0) {
        return Err("'--draws' must be greater than zero".to_owned());
    }
    Ok(SweepOptions {
        seed: seed.ok_or_else(|| "missing required flag '--seed'".to_owned())?,
        draws,
        theta_file,
        noise_mode: noise_mode.unwrap_or(manifest::NoiseMode::Crn),
        ticks: ticks.ok_or_else(|| "missing required flag '--ticks'".to_owned())?,
        population: population.ok_or_else(|| "missing required flag '--population'".to_owned())?,
        out: out.ok_or_else(|| "missing required flag '--out'".to_owned())?,
        params,
        export_pairs,
        timing_json,
        backend: backend.unwrap_or_default(),
        draw_workers,
        enabled_features,
    })
}

pub(crate) fn sweep_file(path: &str, options: SweepOptions) -> i32 {
    match sweep_file_result(path, options) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

#[derive(Debug)]
pub(crate) struct ThetaFile {
    pub(super) assignments: Vec<Vec<ParamOverride>>,
    pub(super) sha256: String,
}

pub(crate) fn read_theta_file(
    model: &sembla_ir::ValidatedModel,
    path: &str,
) -> Result<ThetaFile, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{path}: {error}"))?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|error| format!("{path}: {error}"))?;
    let entries = value
        .as_array()
        .ok_or_else(|| format!("{path}: theta file must be a JSON array"))?;
    if entries.is_empty() {
        return Err(format!(
            "{path}: theta file must contain at least one theta assignment"
        ));
    }
    u32::try_from(entries.len())
        .map_err(|_| format!("{path}: theta file contains more than u32::MAX assignments"))?;

    let mut assignments = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let object = entry
            .as_object()
            .ok_or_else(|| format!("{path}: theta assignment {index} must be a JSON object"))?;
        for declaration in model
            .model()
            .params
            .iter()
            .filter(|declaration| declaration.prior.is_some())
        {
            if !object.contains_key(&declaration.name) {
                return Err(format!(
                    "{path}: theta assignment {index} is missing prior-bearing parameter '{}'",
                    declaration.name
                ));
            }
        }

        let mut overrides = Vec::with_capacity(object.len());
        for (name, value) in object {
            let declaration = model
                .model()
                .params
                .iter()
                .find(|parameter| parameter.name == *name)
                .ok_or_else(|| {
                    format!("{path}: theta assignment {index} has unknown parameter '{name}'")
                })?;
            let value = param_value_from_json(
                declaration,
                value,
                &format!("{path}: theta assignment {index}"),
            )?;
            overrides.push(ParamOverride::new(name, value));
        }
        ParamEnv::resolve(model, &overrides)
            .map_err(|error| format!("{path}: theta assignment {index}: {error}"))?;
        assignments.push(overrides);
    }

    Ok(ThetaFile {
        assignments,
        sha256: hex(&Sha256::digest(&bytes)),
    })
}

pub(crate) fn params_from_theta_assignment(
    model: &sembla_ir::ValidatedModel,
    path: &str,
    draw: u32,
    assignment: &[ParamOverride],
    pinned: &[ParamOverride],
) -> Result<ParamEnv, String> {
    for supplied in assignment {
        if pinned.iter().any(|pin| pin.name == supplied.name) {
            return Err(format!(
                "{path}: theta assignment {draw} parameter '{}' is also supplied by --params",
                supplied.name
            ));
        }
    }
    let mut overrides = Vec::with_capacity(pinned.len() + assignment.len());
    overrides.extend_from_slice(pinned);
    overrides.extend_from_slice(assignment);
    ParamEnv::resolve(model, &overrides)
        .map_err(|error| format!("{path}: theta assignment {draw}: {error}"))
}
