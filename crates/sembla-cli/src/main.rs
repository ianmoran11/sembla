const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let exit_code = run(&arguments);
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}

fn run(arguments: &[String]) -> i32 {
    match arguments {
        [flag] if flag == "--version" => {
            println!("sembla {VERSION}");
            0
        }
        [command, path] if command == "validate" => validate_file(path),
        _ => {
            eprintln!("usage: sembla --version | sembla validate <path>");
            2
        }
    }
}

fn validate_file(path: &str) -> i32 {
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("{path}: {error}");
            return 1;
        }
    };
    let model = match sembla_ir::parse_json(&source) {
        Ok(model) => model,
        Err(error) => {
            eprintln!("{path}: {error}");
            return 1;
        }
    };
    if let Err(error) = sembla_ir::validate(model) {
        eprintln!("{path}: {error}");
        return 1;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::{run, VERSION};

    #[test]
    fn version_matches_library_versions() {
        assert_eq!(VERSION, sembla_ir::VERSION);
        assert_eq!(VERSION, sembla_runtime::VERSION);
    }

    #[test]
    fn invalid_usage_is_nonzero() {
        assert_eq!(run(&[]), 2);
    }
}
