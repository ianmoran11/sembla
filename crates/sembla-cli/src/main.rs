const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let mut args = std::env::args_os();
    let _program = args.next();

    if args.next().as_deref() == Some(std::ffi::OsStr::new("--version")) {
        println!("sembla {VERSION}");
    } else {
        println!("Sembla simulation framework");
    }

    // Keep both workspace libraries linked while the CLI is still a scaffold.
    let _library_versions = (sembla_ir::VERSION, sembla_runtime::VERSION);
}

#[cfg(test)]
mod tests {
    use super::VERSION;

    #[test]
    fn version_matches_library_versions() {
        assert_eq!(VERSION, sembla_ir::VERSION);
        assert_eq!(VERSION, sembla_runtime::VERSION);
    }
}
