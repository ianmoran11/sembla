//! Parameter resolution and stochastic identity for individual sweep draws.

use super::*;

pub(super) fn prepare_draw(
    model: &sembla_ir::ValidatedModel,
    options: &SweepOptions,
    theta_file: Option<&ThetaFile>,
    pinned: &[ParamOverride],
    draw: u32,
) -> Result<SweepPreparedDraw, String> {
    let params = match theta_file {
        Some(theta) => params_from_theta_assignment(
            model,
            options.theta_file.as_deref().expect("theta path exists"),
            draw,
            &theta.assignments[draw as usize],
            pinned,
        )?,
        None => sample_parameters_for_draw(model, options.seed, draw, pinned)
            .map_err(|error| format!("draw {draw}: {error}"))?,
    };
    let execution_seed = match options.noise_mode {
        manifest::NoiseMode::Crn => options.seed,
        manifest::NoiseMode::Independent => derive_sweep_replica_seed(options.seed, draw),
    };
    Ok(SweepPreparedDraw {
        k: draw,
        params,
        execution_seed,
    })
}

pub(super) fn append_parameter_manifest_row(csv: &mut String, draw: &SweepPreparedDraw) {
    csv.push_str(&draw.k.to_string());
    for (_, value) in draw.params.values() {
        csv.push(',');
        csv.push_str(&param_value_csv(value));
    }
    csv.push('\n');
}

pub(super) fn prepare_draws(
    model: &sembla_ir::ValidatedModel,
    options: &SweepOptions,
    theta_file: Option<&ThetaFile>,
    pinned: &[ParamOverride],
    draw_count: u32,
    csv_manifest: &mut String,
) -> Result<Vec<SweepPreparedDraw>, String> {
    (0..draw_count)
        .map(|draw| {
            let prepared = prepare_draw(model, options, theta_file, pinned, draw)?;
            append_parameter_manifest_row(csv_manifest, &prepared);
            Ok(prepared)
        })
        .collect()
}
