use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rustc-check-cfg=cfg(sembla_cuda_toolkit)");
    println!("cargo:rerun-if-changed=src/cuda/f64_native.cu");
    println!("cargo:rerun-if-env-changed=NVCC");
    println!("cargo:rerun-if-env-changed=CUDA_HOME");
    println!("cargo:rerun-if-env-changed=PATH");

    if env::var_os("CARGO_FEATURE_CUDA").is_none() {
        return;
    }

    let Some(nvcc) = find_nvcc() else {
        println!("cargo:warning=CUDA feature enabled but nvcc was not found; cuda: toolkit-absent");
        return;
    };
    let version_status = Command::new(&nvcc)
        .arg("--version")
        .status()
        .unwrap_or_else(|error| {
            panic!(
                "detected nvcc at {}, but it could not be executed: {error}",
                nvcc.display()
            )
        });
    assert!(
        version_status.success(),
        "detected nvcc at {}, but `nvcc --version` failed with {version_status}",
        nvcc.display()
    );

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo did not set OUT_DIR"));
    let library = out_dir.join("libsembla_cuda_f64.a");
    let status = Command::new(&nvcc)
        .args([
            "--lib",
            "-O3",
            "-std=c++14",
            "--fmad=false",
            "--prec-div=true",
            "--prec-sqrt=true",
            "-Xcompiler=-fPIC",
            "-gencode=arch=compute_70,code=[sm_70,compute_70]",
            "src/cuda/f64_native.cu",
            "-o",
        ])
        .arg(&library)
        .status()
        .unwrap_or_else(|error| {
            panic!(
                "failed to launch detected nvcc at {}: {error}",
                nvcc.display()
            )
        });
    assert!(
        status.success(),
        "detected CUDA toolkit, but native f64 kernel compilation failed"
    );

    println!("cargo:rustc-cfg=sembla_cuda_toolkit");
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=sembla_cuda_f64");
    if let Some(toolkit_root) = nvcc.parent().and_then(|bin| bin.parent()) {
        for directory in [toolkit_root.join("lib64"), toolkit_root.join("lib/x64")] {
            if directory.is_dir() {
                println!("cargo:rustc-link-search=native={}", directory.display());
            }
        }
    }
    println!("cargo:rustc-link-lib=dylib=cudart");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-lib=dylib=stdc++");
    }
}

fn find_nvcc() -> Option<PathBuf> {
    if let Some(path) = env::var_os("NVCC").map(PathBuf::from) {
        return Some(path);
    }
    if let Some(home) = env::var_os("CUDA_HOME") {
        let candidate = PathBuf::from(home).join("bin").join(executable("nvcc"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    env::var_os("PATH").and_then(|path| {
        env::split_paths(&path)
            .map(|directory| directory.join(executable("nvcc")))
            .find(|candidate| candidate.is_file())
    })
}

fn executable(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}
