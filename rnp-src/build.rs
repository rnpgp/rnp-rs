// rnp-src build script.
//
// Downloads and compiles librnp + Botan + json-c + zlib + bzip2 from
// source. During `cargo publish --verify`, detects the packaging
// environment (Cargo.toml.orig present) and skips compilation so
// verification is fast. When used as a real dependency by rnp-rs,
// the full compilation runs.
//
// Pure logic (URL composition, dep registry, links contract) lives in
// src/links.rs, shared with lib.rs via `#[path]` so it's unit-testable.
// The full module is imported here, even though build.rs only uses a
// subset (CmakeDep, Deps, JSON_C, ZLIB); the rest is exercised by
// lib.rs's test suite. `#[allow(dead_code)]` silences the unused-item
// warnings for the parts build.rs doesn't touch.

#[path = "src/links.rs"]
#[allow(dead_code)]
mod links;

// Botan version abstraction: botan-src 0.30701 (Botan 3.7.1) and 0.31200
// (Botan 3.12) have slightly different public APIs. We hide that behind
// a local `botan` module so the rest of build.rs can call `botan::build()`
// and `botan::VERSION` uniformly.
//
// PQC + crypto-refresh force the older (3.7.1) Botan because librnp 0.18.1
// references EC_Group / EC_Point by value, which Botan 3.11+ made opaque.
#[cfg(any(feature = "pqc", feature = "crypto-refresh"))]
mod botan {
    use std::path::PathBuf;
    pub const VERSION: &str = "3.7.1";
    pub fn build() -> (String, PathBuf) {
        let (build_dir, include_dir) = botan_src_compat::build();
        (build_dir, PathBuf::from(include_dir))
    }
}

#[cfg(not(any(feature = "pqc", feature = "crypto-refresh")))]
mod botan {
    use std::path::PathBuf;
    pub const VERSION: &str = botan_src::BOTAN_VERSION;
    pub fn build() -> (String, PathBuf) {
        botan_src::build()
    }
}

use links::{CmakeDep, Deps, JSON_C, ZLIB};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const RNP_VERSION: &str = "0.18.1";
const BZIP2_VERSION: &str = "1.0.8";

/// During `cargo publish --verify`, cargo extracts the package to
/// target/package/<name>-<version>/ which contains Cargo.toml.orig.
/// Real builds (from source or as a dependency) don't have this file.
fn is_packaging() -> bool {
    PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_default())
        .join("Cargo.toml.orig")
        .exists()
}

fn main() {
    // Skip heavy compilation during cargo publish --verify.
    // The CI smoke build already verifies correctness.
    if is_packaging() {
        println!("cargo:rustc-env=RNP_SRC_VERSION={RNP_VERSION}");
        println!("cargo:lib_dir=");
        println!("cargo:include_dir=");
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let src_dir = out_dir.join("src");
    let prefix = out_dir.join("install");
    fs::create_dir_all(&src_dir).ok();
    fs::create_dir_all(&prefix).ok();

    // Set macOS deployment target for consistent ABI compatibility.
    // env::set_var is unsafe in Rust 2024 edition.
    if cfg!(target_os = "macos") {
        unsafe {
            env::set_var("MACOSX_DEPLOYMENT_TARGET", "11.0");
        }
    }

    // PQC / crypto-refresh: botan-src reads BOTAN_CONFIGURE_* env vars
    // and forwards them as configure.py flags. Setting ENABLE_MODULES
    // here makes the post-quantum algorithms available; rnp's cmake
    // build then enables ENABLE_PQC=ON / ENABLE_CRYPTO_REFRESH=ON
    // (configured in build_librnp via cfg!).
    if cfg!(feature = "pqc") {
        eprintln!("rnp-src: enabling PQC modules in Botan build");
        unsafe {
            env::set_var(
                "BOTAN_CONFIGURE_ENABLE_MODULES",
                "ml_kem,ml_dsa,slh_dsa_sha2,slh_dsa_shake",
            );
        }
    }

    // --- 1. Botan (via botan-src crate) ---
    eprintln!("rnp-src: building Botan via botan-src crate...");
    let botan_prefix = build_botan(&prefix);

    // --- 2/3. cmake-based deps (json-c, zlib) ---
    let jsonc_prefix = prefix.join("json-c");
    if !jsonc_prefix.join("lib").join("libjson-c.a").exists() {
        eprintln!("rnp-src: building json-c {}...", JSON_C.version);
        cmake_dep_build(&JSON_C, &src_dir, &jsonc_prefix);
    }

    let zlib_prefix = prefix.join("zlib");
    if !zlib_prefix.join("lib").join("libz.a").exists() {
        eprintln!("rnp-src: building zlib {}...", ZLIB.version);
        cmake_dep_build(&ZLIB, &src_dir, &zlib_prefix);
    }

    // --- 4. bzip2 (manual make + bz_internal_error shim) ---
    let bzip2_prefix = prefix.join("bzip2");
    if !bzip2_prefix.join("lib").join("libbz2.a").exists() {
        eprintln!("rnp-src: building bzip2 {BZIP2_VERSION}...");
        build_bzip2(&src_dir, &bzip2_prefix);
    }

    // --- 5. librnp ---
    let rnp_prefix = prefix.join("rnp");
    if !rnp_prefix.join("lib").join("librnp.a").exists() {
        let mut deps = Deps::new();
        deps.push("botan", botan_prefix.clone());
        deps.push("jsonc", jsonc_prefix.clone());
        deps.push("zlib", zlib_prefix.clone());
        deps.push("bzip2", bzip2_prefix.clone());

        eprintln!("rnp-src: building librnp {RNP_VERSION}...");
        build_librnp(&src_dir, &rnp_prefix, &deps);
    }

    // Expose paths to rnp-rs's build.rs via Cargo's links mechanism.
    let rnp_lib = rnp_prefix.join("lib");
    let rnp_inc = rnp_prefix.join("include");
    println!("cargo:lib_dir={}", rnp_lib.display());
    println!("cargo:include_dir={}", rnp_inc.display());

    // Per-dep lib dirs — single source of truth in `Deps::emit_cargo_paths`,
    // matched on the consumer side by `rnp_src::links::DEPS`.
    let mut deps = Deps::new();
    deps.push("botan", botan_prefix.clone());
    deps.push("jsonc", jsonc_prefix.clone());
    deps.push("zlib", zlib_prefix.clone());
    deps.push("bzip2", bzip2_prefix.clone());
    deps.emit_cargo_paths();
}

// ---------------------------------------------------------------------
// Botan — built via botan-src, then staged into our prefix via manual
// file copies (skipping the brittle `make install` step).
// ---------------------------------------------------------------------

fn build_botan(prefix: &Path) -> PathBuf {
    let botan_prefix = prefix.join("botan");
    fs::create_dir_all(botan_prefix.join("lib")).ok();
    fs::create_dir_all(botan_prefix.join("include")).ok();

    let (botan_build_dir, _botan_include_dir) = botan::build();

    // Static library.
    let lib_src = PathBuf::from(&botan_build_dir).join("libbotan-3.a");
    if !lib_src.exists() {
        panic!(
            "rnp-src: expected Botan static library at {}, but it was not produced",
            lib_src.display()
        );
    }
    fs::copy(&lib_src, botan_prefix.join("lib").join("libbotan-3.a"))
        .expect("rnp-src: failed to copy libbotan-3.a into prefix");

    // Public headers — botan-src places them at
    // {build_dir}/build/include/public/ (note the double `build/`).
    let headers_src = PathBuf::from(&botan_build_dir)
        .join("build")
        .join("include")
        .join("public");
    let headers_dst = botan_prefix.join("include").join("botan-3");
    copy_dir_recursive(&headers_src, &headers_dst)
        .expect("rnp-src: failed to copy Botan public headers into prefix");

    // Generate BotanConfig.cmake from a template. See
    // rnp-src/botan/BotanConfig.cmake.in.
    write_botan_cmake_config(&botan_prefix);

    eprintln!("rnp-src: botan install prefix = {}", botan_prefix.display());
    botan_prefix
}

fn write_botan_cmake_config(botan_prefix: &Path) {
    let cmake_dir = botan_prefix
        .join("lib")
        .join("cmake")
        .join(format!("Botan-{}", botan::VERSION));
    fs::create_dir_all(&cmake_dir).ok();

    let template = include_str!("botan/BotanConfig.cmake.in");
    let prefix_str = botan_prefix.display().to_string();
    let config = template
        .replace("@BOTAN_VERSION@", botan::VERSION)
        .replace("@BOTAN_PREFIX@", &prefix_str);

    fs::write(cmake_dir.join("BotanConfig.cmake"), config)
        .expect("rnp-src: failed to write BotanConfig.cmake");
    fs::write(
        cmake_dir.join("BotanConfigVersion.cmake"),
        format!("set(PACKAGE_VERSION \"{}\")\n", botan::VERSION),
    )
    .expect("rnp-src: failed to write BotanConfigVersion.cmake");
}

// ---------------------------------------------------------------------
// Generic cmake dep builder. json-c and zlib are config-driven via
// `CmakeDep`; this is the single place that knows how to invoke cmake.
// Adding a new cmake-based dep = one `CmakeDep` const + a `Deps::push`
// call in main(); no new function.
// ---------------------------------------------------------------------

fn cmake_dep_build(dep: &CmakeDep, src_root: &Path, prefix: &Path) {
    let src = dep.source_dir(src_root);
    if !src.exists() {
        download_and_extract(&dep.url(), src_root);
    }

    let build_dir = dep.build_dir(src_root);

    let mut configure = Command::new("cmake");
    configure
        .args([
            "-S",
            src.to_str().unwrap(),
            "-B",
            build_dir.to_str().unwrap(),
        ])
        .args(["-DCMAKE_BUILD_TYPE=Release", "-DBUILD_SHARED_LIBS=OFF"])
        .args(dep.extra_cmake_args)
        .arg(format!("-DCMAKE_INSTALL_PREFIX={}", prefix.display()));
    if let Some(min) = dep.cmake_policy_minimum {
        configure.arg(format!("-DCMAKE_POLICY_VERSION_MINIMUM={min}"));
    }
    run(&mut configure, &format!("{} cmake", dep.name));

    run(
        Command::new("cmake").args([
            "--build",
            build_dir.to_str().unwrap(),
            "--parallel",
            &nproc(),
        ]),
        &format!("{} build", dep.name),
    );
    run(
        Command::new("cmake").args(["--install", build_dir.to_str().unwrap()]),
        &format!("{} install", dep.name),
    );
}

// ---------------------------------------------------------------------
// bzip2 — hand-rolled because its Makefile is not cmake-compatible
// and it needs the bz_internal_error shim.
// ---------------------------------------------------------------------

fn build_bzip2(src_dir: &Path, prefix: &Path) {
    let bzip2_src = src_dir.join(format!("bzip2-{BZIP2_VERSION}"));
    if !bzip2_src.exists() {
        let url = format!("https://sourceware.org/pub/bzip2/bzip2-{BZIP2_VERSION}.tar.gz");
        download_and_extract(&url, src_dir);
    }

    let cc = if cfg!(target_os = "macos") {
        "/usr/bin/clang"
    } else {
        "gcc"
    };

    run(
        Command::new("make")
            .args(["libbz2.a"])
            .args(["-j", &nproc()])
            .args([format!("CC={cc}"), "CFLAGS=-O3 -fPIC".to_string()])
            .current_dir(&bzip2_src),
        "bzip2 make",
    );

    // Fix: bzip2's Makefile doesn't define bz_internal_error, leaving an
    // undefined symbol in libbz2.a. Write a small .c file, compile it,
    // and append the .o to the archive.
    let shim_src = bzip2_src.join("bz_internal_error_shim.c");
    fs::write(
        &shim_src,
        "#include <stdlib.h>\nvoid bz_internal_error(int errcode) { (void)errcode; abort(); }\n",
    )
    .unwrap();
    run(
        Command::new(cc)
            .args(["-c", "-O3", "-fPIC"])
            .arg(&shim_src)
            .arg("-o")
            .arg(bzip2_src.join("bz_internal_error_shim.o"))
            .current_dir(&bzip2_src),
        "bzip2 bz_internal_error shim compile",
    );
    run(
        Command::new("ar")
            .args(["rcs", "libbz2.a", "bz_internal_error_shim.o"])
            .current_dir(&bzip2_src),
        "bzip2 append shim to libbz2.a",
    );

    fs::create_dir_all(prefix.join("lib")).ok();
    fs::create_dir_all(prefix.join("include")).ok();
    fs::copy(
        bzip2_src.join("libbz2.a"),
        prefix.join("lib").join("libbz2.a"),
    )
    .unwrap();
    fs::copy(
        bzip2_src.join("bzlib.h"),
        prefix.join("include").join("bzlib.h"),
    )
    .unwrap();
}

// ---------------------------------------------------------------------
// librnp — the final consumer of all deps above.
// ---------------------------------------------------------------------

fn build_librnp(src_dir: &Path, prefix: &Path, deps: &Deps) {
    let rnp_src = src_dir.join(format!("rnp-v{RNP_VERSION}"));
    if !rnp_src.exists() {
        let url = format!(
            "https://github.com/rnpgp/rnp/releases/download/v{RNP_VERSION}/rnp-v{RNP_VERSION}.tar.gz"
        );
        download_and_extract(&url, src_dir);
    }

    let (cc, cxx) = if cfg!(target_os = "macos") {
        ("/usr/bin/clang", "/usr/bin/clang++")
    } else {
        ("gcc", "g++")
    };

    let build_dir = src_dir.join("rnp-build");
    let mut cmd = Command::new("cmake");
    cmd.args([
        "-S",
        rnp_src.to_str().unwrap(),
        "-B",
        build_dir.to_str().unwrap(),
    ])
    .args([
        format!("-DCMAKE_C_COMPILER={cc}"),
        format!("-DCMAKE_CXX_COMPILER={cxx}"),
    ])
    .args(["-DCRYPTO_BACKEND=botan3"])
    .args([
        "-DBUILD_SHARED_LIBS=OFF",
        "-DBUILD_TESTING=OFF",
        "-DENABLE_DOC=OFF",
    ])
    .args(["-DCMAKE_BUILD_TYPE=Release"])
    .arg("-DCMAKE_CXX_FLAGS=-include cstring")
    .arg(format!("-DCMAKE_PREFIX_PATH={}", deps.cmake_prefix_path()))
    .arg(format!("-DCMAKE_INSTALL_PREFIX={}", prefix.display()))
    .arg("-DCMAKE_POLICY_VERSION_MINIMUM=3.5");

    // Optional upstream features: surface as Cargo features on rnp-src so
    // rnp-rs can flip them without changing the build pipeline.
    if cfg!(feature = "pqc") {
        eprintln!("rnp-src: building librnp with ENABLE_PQC=ON");
        cmd.arg("-DENABLE_PQC=ON");
    }
    if cfg!(feature = "crypto-refresh") {
        eprintln!("rnp-src: building librnp with ENABLE_CRYPTO_REFRESH=ON");
        cmd.arg("-DENABLE_CRYPTO_REFRESH=ON");
    }

    if cfg!(target_os = "macos") {
        cmd.arg("-DCMAKE_OSX_DEPLOYMENT_TARGET=11.0");
    }

    run(&mut cmd, "librnp cmake");
    run(
        Command::new("cmake").args([
            "--build",
            build_dir.to_str().unwrap(),
            "--parallel",
            &nproc(),
        ]),
        "librnp build",
    );
    run(
        Command::new("cmake").args(["--install", build_dir.to_str().unwrap()]),
        "librnp install",
    );
}

// ---------------------------------------------------------------------
// Process utilities.
// ---------------------------------------------------------------------

fn nproc() -> String {
    Command::new("nproc")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "4".to_string())
}

fn download_and_extract(url: &str, dest: &Path) {
    let tarball = dest.join("download.tar.gz");
    let status = Command::new("curl")
        .args(["-sL", "-o"])
        .arg(&tarball)
        .arg(url)
        .status()
        .unwrap_or_else(|e| panic!("rnp-src: failed to run curl: {e}"));
    assert!(status.success(), "rnp-src: failed to download {url}");
    let status = Command::new("tar")
        .args(["xzf"])
        .arg(&tarball)
        .arg("-C")
        .arg(dest)
        .status()
        .unwrap_or_else(|e| panic!("rnp-src: failed to run tar: {e}"));
    assert!(
        status.success(),
        "rnp-src: failed to extract tarball from {url}"
    );
    let _ = fs::remove_file(&tarball);
}

/// Capture stderr/stdout into the panic message so future failures
/// are diagnosable. Trade-off: callers lose live streaming; for the
/// long Botan compile, that's already handled by botan-src's own
/// stream-to-cargo. Use this for short configure/install steps where
/// error context matters more than progress visibility.
fn run(cmd: &mut Command, label: &str) {
    let output = cmd
        .output()
        .unwrap_or_else(|e| panic!("rnp-src: failed to spawn {label}: {e}"));
    if !output.status.success() {
        panic!(
            "rnp-src: {label} failed with status {}\n\
             --- command ---\n\
             {cmd:?}\n\
             --- stdout ---\n\
             {}\n\
             --- stderr ---\n\
             {}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

/// Recursively copy `src` into `dst`. Errors on IO failure but tolerates
/// destination-already-exists.
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            let _ = fs::remove_file(&to);
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}
