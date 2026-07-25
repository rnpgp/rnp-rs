// Build script: generate raw FFI bindings via bindgen and link against librnp.
//
// The header resolved here is whatever `#include <rnp/rnp.h>` finds on the
// compiler's include path. To point at a non-system header (e.g. a source
// checkout), set RNP_INCLUDE_DIR.

use std::{env, path::PathBuf};

fn main() {
    // --- Locate the rnp headers -------------------------------------------
    //
    // Search order:
    //   1. RNP_INCLUDE_DIR env var (explicit, takes precedence)
    //   2. Homebrew prefix on Apple Silicon (/opt/homebrew/include)
    //   3. Homebrew prefix on Intel (/usr/local/include)
    //   4. Default system include path (/usr/include)
    //
    // We add any located directory to the clang include search path so that
    // `#include <rnp/rnp.h>` resolves.

    let candidate_dirs: Vec<PathBuf> = match env::var("RNP_INCLUDE_DIR") {
        Ok(dir) => vec![PathBuf::from(dir)],
        Err(_) => {
            let mut v = Vec::new();
            if cfg!(target_os = "macos") {
                v.push(PathBuf::from("/opt/homebrew/include"));
                v.push(PathBuf::from("/usr/local/include"));
            }
            v.push(PathBuf::from("/usr/include"));
            v
        }
    };

    let rnp_header = candidate_dirs
        .iter()
        .map(|d| d.join("rnp").join("rnp.h"))
        .find(|p| p.exists())
        .unwrap_or_else(|| {
            panic!(
                "Could not find <rnp/rnp.h>. Install librnp (e.g. `brew install \
                 rnp`) or set RNP_INCLUDE_DIR to the directory containing rnp/rnp.h."
            )
        });

    let rnp_include_dir = rnp_header
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .expect("rnp header path has no grandparent");

    // Locate the matching lib directory (sibling of the include dir on most
    // systems: .../include -> .../lib). Used for the link search path so the
    // linker can find -lrnp on systems where the lib dir isn't searched by
    // default (e.g. Homebrew on Apple Silicon).
    let rnp_lib_dir = match env::var("RNP_LIB_DIR") {
        Ok(dir) => Some(PathBuf::from(dir)),
        Err(_) => {
            // Walk up from the include dir: .../include/rnp/rnp.h -> .../lib
            let mut candidate = rnp_include_dir.clone();
            candidate.pop(); // strip trailing component (e.g. "include")
            candidate.push("lib");
            if candidate.exists() {
                Some(candidate)
            } else {
                None
            }
        }
    };
    if let Some(dir) = &rnp_lib_dir {
        println!("cargo:rustc-link-search=native={}", dir.display());
    }

    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed={}", rnp_header.display());
    println!("cargo:rerun-if-env-changed=RNP_INCLUDE_DIR");

    // --- Generate bindings -------------------------------------------------
    //
    // RNP_USE_64BIT_STRICT: librnp may be built with strict 64-bit time/size
    // handling. Defining it here matches the header's expectations when the
    // library was compiled that way; harmless otherwise.
    //
    // -Wno-deprecated-declarations: rnp_enable_debug/rnp_disable_debug are
    // marked RNP_DEPRECATED; bindgen emits these as warnings which can break
    // the build under -Werror-equivalent defaults.
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", rnp_include_dir.display()))
        .clang_arg("-DRNP_USE_64BIT_STRICT")
        .clang_arg("-Wno-deprecated-declarations")
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

    // --- Link against librnp ----------------------------------------------
    //
    // TODO: when the `vendored` feature is enabled, build librnp from a git
    // submodule here instead of linking the system library.
    println!("cargo:rustc-link-lib=dylib=rnp");
}
