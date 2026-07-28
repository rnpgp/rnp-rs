// Build script: generate raw FFI bindings via bindgen and link against librnp.
//
// Three linking modes, dispatched at compile time via cfg(feature):
//
//   1. --features vendored — delegate to the `rnp-src` crate, which
//      compiles librnp + Botan + json-c + zlib + bzip2 from source and
//      emits DEP_RNP_* env vars.
//   2. RNP_INCLUDE_DIR / RNP_LIB_DIR env vars (explicit path).
//   3. Default — pkg-config discovers the system librnp; falls back to
//      hardcoded candidate paths on minimal images.

use std::env;
use std::path::PathBuf;

#[cfg(feature = "vendored")]
use rnp_src::links;

fn main() {
    let loc = locate_librnp();

    let rnp_header = loc.include_dir.join("rnp").join("rnp.h");
    if !rnp_header.exists() {
        panic!(
            "Could not find <rnp/rnp.h> under {}. Expected the header at {}.",
            loc.include_dir.display(),
            rnp_header.display()
        );
    }

    if let Some(dir) = &loc.lib_dir {
        println!("cargo:rustc-link-search=native={}", dir.display());
    }
    for dir in &loc.extra_lib_dirs {
        println!("cargo:rustc-link-search=native={}", dir.display());
    }
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed={}", rnp_header.display());
    println!("cargo:rerun-if-env-changed=RNP_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=RNP_LIB_DIR");

    let bindings = generate_bindings(&loc.include_dir);

    let out_path = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings");

    emit_link_directives(&loc);
}

// -----------------------------------------------------------------------
// Link-mode resolution.
// -----------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // not every variant is constructed under every feature combo
enum LinkMode {
    /// `cargo:rustc-link-lib=dylib=rnp` — system-installed shared library.
    System,
    /// Same as System but driven by explicit RNP_INCLUDE_DIR/RNP_LIB_DIR.
    Explicit,
    /// rnp-src compiled librnp + all deps as static libraries.
    Vendored,
}

#[derive(Debug, Clone)]
struct LibrnpLocation {
    include_dir: PathBuf,
    lib_dir: Option<PathBuf>,
    link_mode: LinkMode,
    /// Additional `-L` paths emitted via cargo:rustc-link-search. Empty
    /// for System/Explicit; populated by Vendored to surface per-dep
    /// install prefixes (Botan, json-c, zlib, bzip2).
    extra_lib_dirs: Vec<PathBuf>,
    /// Library names to link, with mode-appropriate prefix. When non-empty,
    /// `emit_link_directives` iterates these instead of falling back to
    /// the single hardcoded `dylib=rnp`. Populated by pkg-config so
    /// transitive deps (libz, libbz2, libbotan-3) get linked even when
    /// the distro's static `librnp.a` lacks recorded dependencies.
    extra_link_libs: Vec<LinkLib>,
}

#[derive(Debug, Clone)]
struct LinkLib {
    name: String,
    kind: LinkLibKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // only constructed in the non-vendored build path
enum LinkLibKind {
    Dylib,
    Static,
}

/// Per-feature dispatch: under `--features vendored` we delegate to
/// `locate_vendored`; otherwise to `locate_system_or_explicit`. Splitting
/// by cfg avoids the early-return + `#[allow(unreachable_code)]` pattern
/// and lets the dead-code analyzer prove every variant is constructed.
#[cfg(feature = "vendored")]
fn locate_librnp() -> LibrnpLocation {
    locate_vendored()
}

#[cfg(not(feature = "vendored"))]
fn locate_librnp() -> LibrnpLocation {
    locate_system_or_explicit()
}

#[cfg(feature = "vendored")]
fn locate_vendored() -> LibrnpLocation {
    let lib_dir = PathBuf::from(
        env::var("DEP_RNP_LIB_DIR")
            .expect("DEP_RNP_LIB_DIR not set — rnp-src's build.rs didn't emit lib_dir"),
    );
    let include_dir =
        PathBuf::from(env::var("DEP_RNP_INCLUDE_DIR").expect("DEP_RNP_INCLUDE_DIR not set"));
    // rnp-src emits one cargo:<name>_lib_dir= per dep in `links::DEPS`.
    // Single source of truth — adding a dep there automatically flows
    // through to the linker search path here.
    let extra_lib_dirs: Vec<PathBuf> = links::DEPS
        .iter()
        .filter_map(|&name| env::var(links::lib_dir_env_var(name)).ok())
        .map(PathBuf::from)
        .collect();
    LibrnpLocation {
        include_dir,
        lib_dir: Some(lib_dir),
        link_mode: LinkMode::Vendored,
        extra_lib_dirs,
        extra_link_libs: Vec::new(),
    }
}

#[cfg(not(feature = "vendored"))]
fn locate_system_or_explicit() -> LibrnpLocation {
    // Explicit path via env vars wins over pkg-config / hardcoded search.
    if let Some(loc) = locate_explicit() {
        return loc;
    }
    if let Some(loc) = locate_via_pkg_config() {
        return loc;
    }
    locate_via_hardcoded_paths()
}

#[cfg(not(feature = "vendored"))]
fn locate_explicit() -> Option<LibrnpLocation> {
    let dir = env::var("RNP_INCLUDE_DIR").ok()?;
    let include_dir = PathBuf::from(dir);
    let lib_dir = env::var("RNP_LIB_DIR").ok().map(PathBuf::from).or_else(|| {
        // Sibling-OF include dir: if include is /foo/include, try /foo/lib.
        let mut candidate = include_dir.clone();
        candidate.pop();
        candidate.push("lib");
        if candidate.exists() {
            Some(candidate)
        } else {
            None
        }
    });
    Some(LibrnpLocation {
        include_dir,
        lib_dir,
        link_mode: LinkMode::Explicit,
        extra_lib_dirs: Vec::new(),
        extra_link_libs: Vec::new(),
    })
}

/// Ask pkg-config where librnp + its transitive deps live. Falls back
/// gracefully on images without librnp.pc (returns None → caller falls
/// through to hardcoded path search).
#[cfg(not(feature = "vendored"))]
fn locate_via_pkg_config() -> Option<LibrnpLocation> {
    // Use `--static` so Requires.private deps (libz, libbz2, …) come
    // back too. This matters on macOS where brew's static `librnp.a`
    // has unresolved symbols for its transitive deps — the .dylib
    // records them via install_name but .a does not.
    let lib = pkg_config::Config::new().statik(true).probe("rnp").ok()?;
    let include_dir = lib.include_paths.first()?.clone();
    let lib_dir = lib.link_paths.first().cloned();
    // pkg-config's `libs` field carries bare library names (without
    // the `-l` prefix). Convert each into our LinkLib shape so
    // emit_link_directives can iterate uniformly.
    let extra_link_libs: Vec<LinkLib> = lib
        .libs
        .iter()
        .filter_map(|s| parse_pkg_config_lib(s))
        .collect();
    Some(LibrnpLocation {
        include_dir,
        lib_dir,
        link_mode: LinkMode::System,
        extra_lib_dirs: lib.link_paths.clone(),
        extra_link_libs,
    })
}

/// Parse a bare library name from pkg-config's `libs` field into a
/// (name, kind) pair. Returns None for things we can't represent
/// (e.g. framework references, full paths).
///
/// pkg-config's `libs` is a Vec<String> of names like `rnp`, `z`,
/// `bz2` — no `-l` prefix. The static variant is encoded as
/// `lib<name>.a` which we strip back to `<name>`.
#[cfg(not(feature = "vendored"))]
fn parse_pkg_config_lib(s: &str) -> Option<LinkLib> {
    let (kind, name) = match s.strip_prefix("lib").and_then(|n| n.strip_suffix(".a")) {
        Some(basename) => (LinkLibKind::Static, basename.to_string()),
        None => (LinkLibKind::Dylib, s.to_string()),
    };
    Some(LinkLib { name, kind })
}

#[cfg(not(feature = "vendored"))]
fn locate_via_hardcoded_paths() -> LibrnpLocation {
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
    // Homebrew's librnp.a (when brew ships the static archive) has
    // unresolved symbols for its transitive deps — the .dylib records
    // them via install_name but .a does not. Emit them explicitly as a
    // fallback when pkg-config didn't surface them. Linux package
    // managers (.deb, .rpm) record transitive deps in the .so, so this
    // list is macOS-only.
    let extra_link_libs: Vec<LinkLib> = if cfg!(target_os = "macos") {
        ["z", "bz2", "json-c", "botan-3"]
            .iter()
            .map(|&name| LinkLib {
                name: name.to_string(),
                kind: LinkLibKind::Dylib,
            })
            .collect()
    } else {
        Vec::new()
    };
    LibrnpLocation {
        include_dir,
        lib_dir,
        link_mode: LinkMode::System,
        extra_lib_dirs: Vec::new(),
        extra_link_libs,
    }
}

// -----------------------------------------------------------------------
// Bindings generation.
// -----------------------------------------------------------------------

fn generate_bindings(include_dir: &std::path::Path) -> bindgen::Bindings {
    let pqc_on = cfg!(feature = "pqc");
    let crypto_refresh_on = cfg!(feature = "crypto-refresh");

    let mut builder = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", include_dir.display()))
        .clang_arg("-DRNP_USE_64BIT_STRICT")
        .clang_arg("-Wno-deprecated-declarations");

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

    if pqc_on {
        println!("cargo:rustc-cfg=feature_pqc");
    }
    if crypto_refresh_on {
        println!("cargo:rustc-cfg=feature_crypto_refresh");
    }

    bindings
}

// -----------------------------------------------------------------------
// Link directives.
// -----------------------------------------------------------------------

fn emit_link_directives(loc: &LibrnpLocation) {
    match loc.link_mode {
        LinkMode::System | LinkMode::Explicit => {
            if loc.extra_link_libs.is_empty() {
                // No pkg-config info; fall back to plain `dylib=rnp`.
                println!("cargo:rustc-link-lib=dylib=rnp");
            } else {
                for lib in &loc.extra_link_libs {
                    let kind = match lib.kind {
                        LinkLibKind::Dylib => "dylib",
                        LinkLibKind::Static => "static",
                    };
                    println!("cargo:rustc-link-lib={kind}={}", lib.name);
                }
            }
        }
        LinkMode::Vendored => {
            // rnp-src compiled librnp + all deps as static libraries.
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
}
