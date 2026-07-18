use std::path::Path;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(root.join("../../examples/sir.json")).unwrap();
    let model = sembla_ir::validate(sembla_ir::parse_json(&source).unwrap()).unwrap();
    let generated = sembla_cuda::generate(&model).unwrap();
    let output = root.join("tests/fixtures/sir.generated.cu");
    std::fs::create_dir_all(output.parent().unwrap()).unwrap();
    std::fs::write(&output, generated.source).unwrap();
    println!("{}", output.display());
}
