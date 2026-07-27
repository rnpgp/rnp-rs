// Build script: generate raw FFI bindings via bindgen and link against librnp.
//
// Three linking modes:
//
//   1. Default           — link the system-installed librnp (-lrnp).
//   2. --features vendored — build librnp from `vendor/rnp/` via CMake and
//                           statically link the resulting librnp.a. Requires
//                           the `vendor/rnp/` git submodule to be initialized
//                           and Botan + json-c + zlib to be available.
//   3. RNP_INCLUDE_DIR / RNP_LIB_DIR — explicit pointers to a non-system
//                           librnp install (e.g. a PQC-enabled build).
//
// Feature flags passed to bindgen when their respective Cargo feature is on:
//   --features pqc            -> -DRNP_EXPERIMENTAL_PQC
//   --features crypto-refresh -> -DRNP_EXPERIMENTAL_CRYPTO_REFRESH
// Each requires the linked librnp to have been built with the matching
// ENABLE_* CMake option ON.

use std::{env, path::PathBuf};

// PQC requires Botan's ML-KEM/ML-DSA/SLH-DSA modules. The minimal Botan
// variant drops them; OpenSSL 3.x and LibreSSL don't have them at all.
// Fail fast at build time instead of letting callers discover the
// mismatch at runtime.
#[cfg(all(feature = "vendored-minimal", feature = "pqc"))]
compile_error!(
    "`vendored-minimal` and `pqc` are mutually exclusive — the minimal Botan \
     build drops the ML-KEM/ML-DSA/SLH-DSA modules. Use the full `vendored` \
     feature (without `vendored-minimal`) when you need PQC."
);
#[cfg(all(feature = "vendored-openssl3", feature = "pqc"))]
compile_error!(
    "`vendored-openssl3` and `pqc` are mutually exclusive — OpenSSL 3.x does \
     not implement the PQC algorithms (ML-KEM/ML-DSA/SLH-DSA). Use the \
     Botan-backed `vendored` feature when you need PQC."
);

fn main() {
    // ---------------------------------------------------------------------
    // 1. Locate the librnp headers and decide how to link.
    // ---------------------------------------------------------------------

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

    // ---------------------------------------------------------------------
    // 2. Generate bindings via bindgen.
    // ---------------------------------------------------------------------

    let mut builder = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", include_dir.display()))
        // RNP_USE_64BIT_STRICT: librnp may be built with strict 64-bit
        // time/size handling. Defining it here matches the header's
        // expectations when the library was compiled that way; harmless
        // otherwise.
        .clang_arg("-DRNP_USE_64BIT_STRICT")
        // rnp_enable_debug / rnp_disable_debug are RNP_DEPRECATED; bindgen
        // emits warnings that can break the build under -Werror-equivalent
        // defaults.
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
        println!(
            "cargo:warning=rnp-rs: building with RNP_EXPERIMENTAL_CRYPTO_REFRESH — \
             requires librnp built with ENABLE_CRYPTO_REFRESH=ON"
        );
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

    // ---------------------------------------------------------------------
    // 3. Link against librnp.
    // ---------------------------------------------------------------------

    match link_mode {
        LinkMode::System => {
            println!("cargo:rustc-link-lib=dylib=rnp");
        }
        LinkMode::Explicit => {
            // The link-search line above already added lib_dir; just declare
            // the dependency.
            println!("cargo:rustc-link-lib=dylib=rnp");
        }
        LinkMode::Vendored { static_lib_path, backend } => {
            println!("cargo:rustc-link-lib=static=rnp");
            println!("cargo:rustc-link-lib=static=sexpp");
            println!("cargo:rustc-link-lib=static=json-c");
            println!("cargo:rustc-link-lib=static=z");
            println!("cargo:rustc-link-lib=static=bz2");
            println!("cargo:rerun-if-changed={}", static_lib_path.display());
            match backend {
                VendoredBackend::Botan | VendoredBackend::BotanPqc => {
                    println!("cargo:rustc-link-lib=static=botan-3");
                }
                VendoredBackend::OpenSSL3 => {
                    println!("cargo:rustc-link-lib=static=crypto");
                    println!("cargo:rustc-link-lib=static=ssl");
                    if !cfg!(target_os = "macos") {
                        println!("cargo:rustc-link-lib=dylib=dl");
                        println!("cargo:rustc-link-lib=dylib=pthread");
                    }
                }
            }
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
    /// Default: `-lrnp` from a system search path (Homebrew, /usr/lib, ...).
    System,
    /// `RNP_LIB_DIR` (and optionally `RNP_INCLUDE_DIR`) point at a
    /// non-system install.
    Explicit,
    /// `--features vendored`: build librnp from `vendor/rnp/` via CMake.
    /// The path is the resulting `librnp.a`. Only constructed when the
    /// `vendored` Cargo feature is on.
    #[allow(dead_code)]
    Vendored {
        static_lib_path: PathBuf,
        backend: VendoredBackend,
    },
}

/// Crypto backend selected by the vendored-mode feature flag combination.
/// Mirrors the `prebuilt/<target>-<suffix>/` directory layout.
#[allow(dead_code)]
enum VendoredBackend {
    Botan,
    BotanPqc,
    OpenSSL3,
}

impl VendoredBackend {
    /// Subdir suffix appended to the target triple to find the right
    /// prebuilt variant. Empty for the default (Botan full).
    #[allow(dead_code)]
    fn subdir_suffix() -> &'static str {
        if cfg!(feature = "vendored-pqc") {
            "-pqc"
        } else if cfg!(feature = "vendored-openssl3") {
            "-openssl3"
        } else if cfg!(feature = "vendored-minimal") {
            "-minimal"
        } else {
            ""
        }
    }

    #[allow(dead_code)]
    fn from_features() -> Self {
        if cfg!(feature = "vendored-pqc") {
            VendoredBackend::BotanPqc
        } else if cfg!(feature = "vendored-openssl3") {
            VendoredBackend::OpenSSL3
        } else {
            VendoredBackend::Botan
        }
    }
}

fn locate_librnp() -> (PathBuf, Option<PathBuf>, LinkMode) {
    // Vendored has highest precedence.
    #[cfg(feature = "vendored")]
    {
        let (include_dir, lib_dir, static_lib) = build_vendored();
        return (
            include_dir,
            Some(lib_dir),
            LinkMode::Vendored {
                static_lib_path: static_lib,
                backend: VendoredBackend::from_features(),
            },
        );
    }

    // Default branch (compiled when `vendored` is off, or always — the
    // `return` above is conditional on the feature).
    #[allow(unreachable_code)]
    {
        // Explicit RNP_INCLUDE_DIR / RNP_LIB_DIR.
        if let Ok(dir) = env::var("RNP_INCLUDE_DIR") {
            let include_dir = PathBuf::from(dir);
            let lib_dir = env::var("RNP_LIB_DIR").ok().map(PathBuf::from).or_else(|| {
                // Default: assume sibling `lib/` next to the include dir.
                let mut candidate = include_dir.clone();
                candidate.pop(); // strip trailing component ("include")
                candidate.push("lib");
                if candidate.exists() {
                    Some(candidate)
                } else {
                    None
                }
            });
            return (include_dir, lib_dir, LinkMode::Explicit);
        }

        // System search: Homebrew prefixes first on macOS, then /usr/include.
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
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                // Fall back to /usr/include even if missing — bindgen will fail
                // with a clearer message later.
                PathBuf::from("/usr/include")
            });

        let lib_dir = if cfg!(target_os = "macos") {
            // Homebrew installs lib under the include dir's sibling.
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

// -----------------------------------------------------------------------
// Vendored build: invoke CMake on `vendor/rnp/`.
// -----------------------------------------------------------------------

#[cfg(feature = "vendored")]
fn build_vendored() -> (PathBuf, PathBuf, PathBuf) {
    let target = std::env::var("TARGET").unwrap_or_default();
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_default());

    // Backend feature flags select different prebuilt subtrees:
    //   (default)             -> prebuilt/<target>/         (Botan, full)
    //   vendored-minimal      -> prebuilt/<target>-minimal/ (Botan, minimal)
    //   vendored-openssl3     -> prebuilt/<target>-openssl3/
    //   vendored-libressl     -> prebuilt/<target>-libressl/
    let prebuilt_subdir = format!("{}{}", target, VendoredBackend::subdir_suffix());
    let prebuilt_dir = manifest_dir.join("prebuilt").join(&prebuilt_subdir);

    let include_dir = prebuilt_dir.join("include");
    let lib_dir = prebuilt_dir.join("lib");
    let static_lib = lib_dir.join("librnp.a");

    // Fallback: if no prebuilt dir for this target, try the old cmake-from-
    // submodule path.
    if !static_lib.exists() {
        return build_vendored_from_source();
    }

    (include_dir, lib_dir, static_lib)
}

#[cfg(feature = "vendored")]
fn build_vendored_from_source() -> (PathBuf, PathBuf, PathBuf) {
    let vendor_dir = PathBuf::from("vendor/rnp");
    if !vendor_dir.join("CMakeLists.txt").exists() {
        panic!(
            "vendored feature is enabled but no prebuilt libs found for target {} and \
             `vendor/rnp/CMakeLists.txt` is missing. Pre-built static libraries are \
             available for: aarch64-apple-darwin. For other targets, initialize the \
             submodule:\n  \
             git submodule update --init --recursive\n\
             Or drop the `--features vendored` flag to use the system librnp.",
            std::env::var("TARGET").unwrap_or_default()
        );
    }

    // Allow the consumer to pick a backend (default botan).
    let backend = env::var("RNP_VENDOR_BACKEND").unwrap_or_else(|_| "botan".to_string());
    let pqc = if cfg!(feature = "pqc") { "ON" } else { "OFF" };
    let crypto_refresh = if cfg!(feature = "crypto-refresh") {
        "ON"
    } else {
        "OFF"
    };

    let mut config = cmake::Config::new(&vendor_dir);
    config
        .define("CRYPTO_BACKEND", &backend)
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("BUILD_TESTING", "OFF")
        .define("ENABLE_PQC", pqc)
        .define("ENABLE_CRYPTO_REFRESH", crypto_refresh)
        .define("ENABLE_DOC", "OFF")
        .define("CMAKE_BUILD_TYPE", "Release");

    // Allow consumers to pass through extra CMake args.
    if let Ok(extra) = env::var("RNP_VENDOR_CMAKE_ARGS") {
        for arg in extra.split_whitespace() {
            if let Some((k, v)) = arg.split_once('=') {
                config.define(k, v);
            }
        }
    }

    let install_dir = config.build();

    let include_dir = install_dir.join("include");
    let lib_dir = install_dir.join("lib");
    let static_lib = lib_dir.join("librnp.a");
    if !static_lib.exists() {
        panic!(
            "vendored build completed but `librnp.a` not found at {}. \
             Inspect the CMake output above.",
            static_lib.display()
        );
    }

    (include_dir, lib_dir, static_lib)
}
