//! Per-draw artifacts, sweep summaries, and output cleanup.

use super::*;

pub(super) struct SweepPublication<'a> {
    pub(super) run_manifest: manifest::RunManifest,
    pub(super) parameter_columns: Vec<String>,
    pub(super) summary_columns: Vec<String>,
    pub(super) pairs_csv: Option<String>,
    pub(super) reported_columns: Option<Vec<String>>,
    pub(super) all_series: Vec<Vec<Vec<ReportedValue>>>,
    out: &'a Path,
    export_pairs: bool,
}

impl<'a> SweepPublication<'a> {
    pub(super) fn new(
        model: &sembla_ir::ValidatedModel,
        run_manifest: manifest::RunManifest,
        out: &'a Path,
        export_pairs: bool,
        draw_count: u32,
    ) -> Self {
        let mut parameter_columns = model
            .model()
            .params
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect::<Vec<_>>();
        parameter_columns.sort();
        let summary_columns = model
            .model()
            .summaries
            .iter()
            .map(|summary| summary.name.clone())
            .collect::<Vec<_>>();
        let pairs_csv = export_pairs.then(|| {
            let mut columns = vec!["k".to_owned()];
            columns.extend(parameter_columns.iter().cloned());
            columns.extend(summary_columns.iter().cloned());
            let mut csv = columns
                .iter()
                .map(|column| csv_field(column))
                .collect::<Vec<_>>()
                .join(",");
            csv.push('\n');
            csv
        });
        Self {
            run_manifest,
            parameter_columns,
            summary_columns,
            pairs_csv,
            reported_columns: None,
            all_series: Vec::with_capacity(draw_count as usize),
            out,
            export_pairs,
        }
    }

    pub(super) fn publish(
        &mut self,
        prepared: &SweepPreparedDraw,
        execution: SweepDrawOutput,
    ) -> Result<(), String> {
        let draw = prepared.k;
        let output = execution.output;
        if let Some(columns) = self.reported_columns.as_ref() {
            if columns != &output.series.columns {
                return Err(format!(
                    "draw {draw}: reported column schema changed across draws"
                ));
            }
        } else {
            self.reported_columns = Some(output.series.columns.clone());
        }
        if let Some(csv) = &mut self.pairs_csv {
            append_pairs_row(
                csv,
                draw,
                &prepared.params,
                &self.parameter_columns,
                &output.summaries,
                &self.summary_columns,
            )?;
        }
        let hashes = execution_hashes_with_state_hash(&output, execution.final_state_hash);
        let grouped_outputs = grouped_output_records(&output.grouped);
        self.run_manifest
            .executions
            .push(manifest::ManifestExecution {
                k: draw,
                seed: Some(prepared.execution_seed),
                scenario: None,
                model: None,
                ir_hash: None,
                dt: None,
                resolved_theta: manifest::resolved_theta(&prepared.params),
                results_sha256: hashes.results_sha256,
                final_state_sha256: hashes.final_state_sha256,
                observation_sha256: Some(hashes.observation_sha256),
                grouped_outputs,
            });
        let draw_path = self.out.join(format!("draw_{draw}.csv"));
        write_atomic(&draw_path, output.csv.as_bytes())?;
        for grouped in &output.grouped {
            let path = grouped_output_path(&draw_path, &grouped.view);
            write_atomic(&path, grouped.csv.as_bytes())?;
        }
        if self.export_pairs {
            let draw_summaries = PathBuf::from(format!("{}.summaries.csv", draw_path.display()));
            write_atomic(draw_summaries, output.summaries_csv.as_bytes())?;
        }
        self.all_series.push(output.series.rows);
        Ok(())
    }
}

pub(crate) fn remove_previous_sweep_outputs(directory: &Path) -> Result<(), String> {
    for entry in
        std::fs::read_dir(directory).map_err(|error| format!("{}: {error}", directory.display()))?
    {
        let path = entry
            .map_err(|error| format!("{}: {error}", directory.display()))?
            .path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if name == "manifest.csv"
            || name == "run-manifest.json"
            || name == "summary.csv"
            || (name.starts_with("draw_") && name.ends_with(".csv"))
        {
            std::fs::remove_file(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        }
    }
    Ok(())
}

pub(crate) fn param_value_csv(value: &ParamValue) -> String {
    match value {
        ParamValue::Real { value } => value.to_string(),
        ParamValue::Int { value } => value.to_string(),
    }
}

pub(crate) fn append_pairs_row(
    csv: &mut String,
    draw: u32,
    params: &ParamEnv,
    parameter_columns: &[String],
    summaries: &[SummaryValue],
    summary_columns: &[String],
) -> Result<(), String> {
    let mut values = params.values().collect::<Vec<_>>();
    values.sort_by(|left, right| left.0.cmp(right.0));
    if values
        .iter()
        .map(|(name, _)| *name)
        .ne(parameter_columns.iter().map(String::as_str))
    {
        return Err(format!(
            "draw {draw}: resolved parameter columns do not match the export schema"
        ));
    }
    if summaries
        .iter()
        .map(|summary| summary.name.as_str())
        .ne(summary_columns.iter().map(String::as_str))
    {
        return Err(format!(
            "draw {draw}: summary columns do not match model declaration order"
        ));
    }

    csv.push_str(&draw.to_string());
    for (_, value) in values {
        csv.push(',');
        csv.push_str(&param_value_csv(value));
    }
    for summary in summaries {
        csv.push(',');
        csv.push_str(&ReportedValue::from(summary.value).csv());
    }
    csv.push('\n');
    Ok(())
}

pub(crate) fn summary_csv(
    columns: &[String],
    all_series: &[Vec<Vec<ReportedValue>>],
    ticks: u32,
) -> Result<String, String> {
    const PERCENTILES: [usize; 5] = [5, 25, 50, 75, 95];
    let mut csv = String::from("tick");
    for name in columns {
        for percentile in PERCENTILES {
            csv.push(',');
            csv.push_str(&csv_field(&format!("{name}_p{percentile:02}")));
        }
    }
    csv.push('\n');
    for tick in 0..ticks as usize {
        csv.push_str(&tick.to_string());
        for (column, column_name) in columns.iter().enumerate() {
            let mut values = all_series
                .iter()
                .map(|series| {
                    series
                        .get(tick)
                        .and_then(|row| row.get(column))
                        .copied()
                        .ok_or_else(|| {
                            format!("reported series is missing tick {tick} column '{column_name}'")
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if let Some(first) = values.first().copied() {
                for value in values.iter().skip(1).copied() {
                    value.cmp(first)?;
                }
            }
            values.sort_by(|left, right| {
                left.cmp(*right)
                    .expect("reported column type was checked before sorting")
            });
            for percentile in PERCENTILES {
                // Deterministic nearest index to p * (n - 1).
                let index = ((values.len() - 1) * percentile + 50) / 100;
                csv.push(',');
                csv.push_str(&values[index].csv());
            }
        }
        csv.push('\n');
    }
    Ok(csv)
}
