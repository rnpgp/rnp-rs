// Build script: generate raw FFI bindings via bindgen and link against librnp.
//
// Two linking modes:
//
//   1. Default — link the system-installed librnp (-lrnp).
//   2. --features vendored — delegate to the `rnp-src` crate, which
//      compiles librnp + Botan + json-c + zlib + bzip2 from source.

use std::env;
use std::path::PathBuf;

fn main() {
    let (include_dir, lib_dir, link_mode) = locate_librnp();

    let rnp_header = include_dir.join("rnp").join("rnp.h");
    if !rnp_header.exists() {
        panic!(
            "Could not find <rnp/rnp.h> under {}. Expected the header at {}.",
            include_dir.display(),
            rnp_header.display()
        );
    }

    if let Some(dir) = &lib_dir {
        println!("cargo:rustc-link-search=native={}", dir.display());
    }
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed={}", rnp_header.display());
    println!("cargo:rerun-if-env-changed=RNP_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=RNP_LIB_DIR");

    // Generate bindings via bindgen.
    let mut builder = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", include_dir.display()))
        .clang_arg("-DRNP_USE_64BIT_STRICT")
        .clang_arg("-Wno-deprecated-declarations");

    let pqc_on = cfg!(feature = "pqc");
    let crypto_refresh_on = cfg!(feature = "crypto-refresh");
    if pqc_on {
        println!(
            "cargo:warning=rnp-rs: building with RNP_EXPERIMENTAL_PQC — requires \
             librnp built with ENABLE_PQC=ON"
        );
        builder = builder.clang_arg("-DRNP_EXPERIMENTAL_PQC");
    }
    if crypto_refresh_on {
        builder = builder.clang_arg("-DRNP_EXPERIMENTAL_CRYPTO_REFRESH");
    }

    let bindings = builder
        .allowlist_function("rnp_.*")
        .allowlist_type("rnp_.*")
        .allowlist_var("RNP_.*")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .layout_tests(false)
        .default_macro_constant_type(bindgen::MacroTypeVariation::Signed)
        .generate()
        .expect("Unable to generate rnp bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings");

    // Link against librnp.
    match link_mode {
        LinkMode::System | LinkMode::Explicit => {
            println!("cargo:rustc-link-lib=dylib=rnp");
        }
        LinkMode::Vendored => {
            // rnp-src compiled librnp + all deps as static libraries.
            // We link against them here.
            println!("cargo:rustc-link-lib=static=rnp");
            println!("cargo:rustc-link-lib=static=sexpp");
            println!("cargo:rustc-link-lib=static=botan-3");
            println!("cargo:rustc-link-lib=static=json-c");
            println!("cargo:rustc-link-lib=static=z");
            println!("cargo:rustc-link-lib=static=bz2");
            if cfg!(target_os = "macos") {
                println!("cargo:rustc-link-lib=dylib=c++");
            } else {
                println!("cargo:rustc-link-lib=dylib=stdc++");
            }
        }
    }

    if pqc_on {
        println!("cargo:rustc-cfg=feature_pqc");
    }
    if crypto_refresh_on {
        println!("cargo:rustc-cfg=feature_crypto_refresh");
    }
}

// -----------------------------------------------------------------------
// Link-mode resolution.
// -----------------------------------------------------------------------

enum LinkMode {
    System,
    Explicit,
    Vendored,
}

fn locate_librnp() -> (PathBuf, Option<PathBuf>, LinkMode) {
    // Vendored: delegate to rnp-src crate.
    #[cfg(feature = "vendored")]
    {
        // rnp-src has already compiled everything in its build.rs.
        // Its lib.rs exposes the paths via env!() macros.
        let lib_dir = rnp_src::lib_dir();
        let include_dir = rnp_src::include_dir();
        return (include_dir, Some(lib_dir), LinkMode::Vendored);
    }

    #[allow(unreachable_code)]
    {
        if let Ok(dir) = env::var("RNP_INCLUDE_DIR") {
            let include_dir = PathBuf::from(dir);
            let lib_dir = env::var("RNP_LIB_DIR").ok().map(PathBuf::from).or_else(|| {
                let mut candidate = include_dir.clone();
                candidate.pop();
                candidate.push("lib");
                if candidate.exists() {
                    Some(candidate)
                } else {
                    None
                }
            });
            return (include_dir, lib_dir, LinkMode::Explicit);
        }

        let candidate_dirs: Vec<PathBuf> = if cfg!(target_os = "macos") {
            vec![
                PathBuf::from("/opt/homebrew/include"),
                PathBuf::from("/usr/local/include"),
                PathBuf::from("/usr/include"),
            ]
        } else {
            vec![PathBuf::from("/usr/include")]
        };

        let include_dir = candidate_dirs
            .iter()
            .find(|d| d.join("rnp").join("rnp.h").exists())
            .cloned()
            .unwrap_or_else(|| PathBuf::from("/usr/include"));

        let lib_dir = if cfg!(target_os = "macos") {
            let mut candidate = include_dir.clone();
            candidate.pop();
            candidate.push("lib");
            if candidate.exists() {
                Some(candidate)
            } else {
                None
            }
        } else {
            None
        };

        (include_dir, lib_dir, LinkMode::System)
    }
}
