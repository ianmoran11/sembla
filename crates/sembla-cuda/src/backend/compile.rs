//! One-entry, process-local PTX reuse across independent CUDA backends.

use std::sync::Mutex;

use cudarc::nvrtc::{compile_ptx_with_opts, CompileError, CompileOptions, Ptx};

static LAST_PTX: Mutex<Option<(String, Ptx)>> = Mutex::new(None);

pub(super) fn compile(source: &str, hash: &str) -> Result<(Ptx, bool), CompileError> {
    // The compiler library and options are fixed within this process. Source
    // identity includes execution mode and fused rewriting. Modules and device
    // allocations still belong to each backend's context independently.
    let mut cached = LAST_PTX.lock().unwrap_or_else(|error| error.into_inner());
    reuse_or_compile(&mut cached, hash, || {
        compile_ptx_with_opts(
            source,
            CompileOptions {
                ftz: Some(false),
                prec_div: Some(true),
                prec_sqrt: Some(true),
                fmad: Some(false),
                options: vec!["--std=c++14".to_owned()],
                name: Some(format!("sembla-{hash}.cu")),
                ..Default::default()
            },
        )
    })
}

fn reuse_or_compile<E>(
    cached: &mut Option<(String, Ptx)>,
    hash: &str,
    compile: impl FnOnce() -> Result<Ptx, E>,
) -> Result<(Ptx, bool), E> {
    if let Some((key, ptx)) = cached {
        if key == hash {
            return Ok((ptx.clone(), true));
        }
    }
    let ptx = compile()?;
    *cached = Some((hash.to_owned(), ptx.clone()));
    Ok((ptx, false))
}

#[cfg(test)]
mod tests {
    use super::{reuse_or_compile, Ptx};

    #[test]
    fn cache_reuses_only_matching_source_and_does_not_cache_failures() {
        let mut cached = None;
        let first =
            reuse_or_compile(&mut cached, "first", || Ok::<_, ()>(Ptx::from_src("ptx-a"))).unwrap();
        assert!(!first.1);
        let hit =
            reuse_or_compile::<()>(&mut cached, "first", || panic!("cache hit compiled")).unwrap();
        assert!(hit.1);
        assert_eq!(hit.0.to_src(), "ptx-a");
        assert!(reuse_or_compile::<()>(&mut cached, "second", || Err(())).is_err());
        assert_eq!(cached.as_ref().unwrap().0, "first");
        let second = reuse_or_compile(&mut cached, "second", || {
            Ok::<_, ()>(Ptx::from_src("ptx-b"))
        })
        .unwrap();
        assert!(!second.1);
        assert_eq!(cached.unwrap().0, "second");
    }
}
