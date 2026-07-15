use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn repository_path(relative: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn temp_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "sembla-canonical-models-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

struct Example {
    file: &'static str,
    ticks: &'static str,
    state_columns: usize,
    header: &'static str,
}

const EXAMPLES: &[Example] = &[
    Example {
        file: "reversible_ctmc.json",
        ticks: "20",
        state_columns: 2,
        header: "tick,count:chain.particle.phase=A,count:chain.particle.phase=B,fired:chain.move_ab,fired:chain.move_ba,deferred_total",
    },
    Example {
        file: "radioactive_decay_chain.json",
        ticks: "30",
        state_columns: 3,
        header: "tick,count:decay.atom.nuclide=Parent,count:decay.atom.nuclide=Daughter,count:decay.atom.nuclide=Stable,fired:decay.parent_decay,fired:decay.daughter_decay,deferred_total",
    },
    Example {
        file: "sis_importation.json",
        ticks: "30",
        state_columns: 2,
        header: "tick,count:epidemic.person.health=S,count:epidemic.person.health=I,fired:epidemic.infect,fired:epidemic.recover,deferred_total",
    },
    Example {
        file: "seirs_waning.json",
        ticks: "40",
        state_columns: 4,
        header: "tick,count:epidemic.person.health=S,count:epidemic.person.health=E,count:epidemic.person.health=I,count:epidemic.person.health=R,fired:epidemic.expose,fired:epidemic.progress,fired:epidemic.recover,fired:epidemic.wane,deferred_total",
    },
    Example {
        file: "noisy_voter.json",
        ticks: "30",
        state_columns: 2,
        header: "tick,count:opinions.agent.opinion=A,count:opinions.agent.opinion=B,fired:opinions.adopt_b,fired:opinions.adopt_a,deferred_total",
    },
];

#[test]
fn canonical_models_validate_run_deterministically_and_conserve_rows() {
    let temp = temp_dir();
    for example in EXAMPLES {
        let model = repository_path(format!("examples/{}", example.file));
        let validation = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("validate")
            .arg(&model)
            .output()
            .unwrap();
        assert!(
            validation.status.success(),
            "{}: {}",
            example.file,
            String::from_utf8_lossy(&validation.stderr)
        );

        let first = temp.join(format!("{}-first.csv", example.file));
        let second = temp.join(format!("{}-second.csv", example.file));
        let run = |out: &Path| {
            Command::new(env!("CARGO_BIN_EXE_sembla"))
                .arg("run")
                .arg(&model)
                .args([
                    "--population",
                    "1000",
                    "--seed",
                    "55",
                    "--ticks",
                    example.ticks,
                    "--out",
                ])
                .arg(out)
                .output()
                .unwrap()
        };
        let first_output = run(&first);
        let second_output = run(&second);
        assert!(
            first_output.status.success(),
            "{}: {}",
            example.file,
            String::from_utf8_lossy(&first_output.stderr)
        );
        assert_eq!(
            first_output.stdout, second_output.stdout,
            "{}",
            example.file
        );
        assert_eq!(
            std::fs::read(&first).unwrap(),
            std::fs::read(&second).unwrap()
        );

        let csv = std::fs::read_to_string(&first).unwrap();
        let mut lines = csv.lines();
        assert!(lines.next().unwrap().starts_with("# params="));
        assert!(lines.next().unwrap().starts_with("# dt="));
        assert_eq!(lines.next(), Some(example.header), "{}", example.file);
        let rows = lines
            .map(|line| {
                line.split(',')
                    .map(|value| value.parse::<usize>().unwrap())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), example.ticks.parse::<usize>().unwrap());
        for row in &rows {
            assert_eq!(
                row[1..1 + example.state_columns].iter().sum::<usize>(),
                1000,
                "{} row {row:?}",
                example.file
            );
        }
        assert!(
            rows.last().unwrap()[1] < 1000,
            "{} did not leave homogeneous variant-zero initialization",
            example.file
        );
        let transition_start = 1 + example.state_columns;
        let transition_end = rows[0].len() - 1;
        for column in transition_start..transition_end {
            assert!(
                rows.iter().map(|row| row[column]).sum::<usize>() > 0,
                "{} transition column {column} never fired",
                example.file
            );
        }
        if example.file == "radioactive_decay_chain.json" {
            assert!(rows.windows(2).all(|pair| pair[1][1] <= pair[0][1]));
            assert!(rows.windows(2).all(|pair| pair[1][3] >= pair[0][3]));
        }
    }
    std::fs::remove_dir_all(temp).unwrap();
}
