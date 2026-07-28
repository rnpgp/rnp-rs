// rnp-src build script.
//
// Downloads and compiles librnp + Botan + json-c + zlib + bzip2 from
// source. During `cargo publish --verify`, detects the packaging
// environment (Cargo.toml.orig present) and skips compilation so
// verification is fast. When used as a real dependency by rnp-rs,
// the full compilation runs.

use std::env;
use std::path::PathBuf;
use std::process::Command;

const RNP_VERSION: &str = "0.18.1";
const JSON_C_VERSION: &str = "0.17";
const ZLIB_VERSION: &str = "1.3.1";
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
    std::fs::create_dir_all(&src_dir).ok();
    std::fs::create_dir_all(&prefix).ok();

    // Set macOS deployment target for consistent ABI compatibility.
    // env::set_var is unsafe in Rust 2024 edition.
    if cfg!(target_os = "macos") {
        unsafe {
            env::set_var("MACOSX_DEPLOYMENT_TARGET", "11.0");
        }
    }

    // --- 1. Botan (via botan-src crate) ---
    // botan_src::build() configures and compiles Botan, returning
    // (build_dir, include_dir). We then run `make install` to create
    // a proper prefix that cmake's find_package(Botan) can find.
    eprintln!("rnp-src: building Botan via botan-src crate...");
    let (botan_build_dir, _botan_include_dir) = botan_src::build();
    let botan_prefix = prefix.join("botan");

    // botan_src extracts source to {OUT_DIR}/botan-src/Botan-{VERSION}/
    // (NOT src_dir/botan-src — src_dir is our own download dir).
    let botan_source_dir = out_dir
        .join("botan-src")
        .join(format!("Botan-{}", botan_src::BOTAN_VERSION));
    if botan_source_dir.exists() {
        // Use `prefix=` (not DESTDIR) so files install directly into
        // {botan_prefix}/lib/ and {botan_prefix}/include/ — what cmake
        // expects in CMAKE_PREFIX_PATH.
        run(
            Command::new("make")
                .arg("-f")
                .arg(format!("{botan_build_dir}/Makefile"))
                .arg(format!("prefix={}", botan_prefix.display()))
                .arg("install")
                .current_dir(&botan_source_dir),
            "botan make install",
        );
    } else {
        panic!(
            "rnp-src: botan source not found at {} — botan_src::build() may have changed its extraction path",
            botan_source_dir.display()
        );
    }
    let botan_lib_dir = botan_prefix.join("lib");
    eprintln!("rnp-src: botan install prefix = {}", botan_prefix.display());

    // --- 2. json-c ---
    let jsonc_prefix = prefix.join("json-c");
    if !jsonc_prefix.join("lib").join("libjson-c.a").exists() {
        eprintln!("rnp-src: building json-c {JSON_C_VERSION}...");
        build_jsonc(&src_dir, &jsonc_prefix);
    }

    // --- 3. zlib ---
    let zlib_prefix = prefix.join("zlib");
    if !zlib_prefix.join("lib").join("libz.a").exists() {
        eprintln!("rnp-src: building zlib {ZLIB_VERSION}...");
        build_zlib(&src_dir, &zlib_prefix);
    }

    // --- 4. bzip2 (with bz_internal_error fix) ---
    let bzip2_prefix = prefix.join("bzip2");
    if !bzip2_prefix.join("lib").join("libbz2.a").exists() {
        eprintln!("rnp-src: building bzip2 {BZIP2_VERSION}...");
        build_bzip2(&src_dir, &bzip2_prefix);
    }

    // --- 5. librnp ---
    let rnp_prefix = prefix.join("rnp");
    if !rnp_prefix.join("lib").join("librnp.a").exists() {
        eprintln!("rnp-src: building librnp {RNP_VERSION}...");
        build_librnp(
            &src_dir,
            &rnp_prefix,
            &botan_prefix,
            &jsonc_prefix,
            &zlib_prefix,
            &bzip2_prefix,
        );
    }

    // Expose paths to rnp-rs's build.rs via Cargo's links mechanism.
    // rnp-src declares `links = "rnp"` in Cargo.toml, so these become
    // DEP_RNP_LIB_DIR, DEP_RNP_INCLUDE_DIR, etc. in downstream build.rs.
    let rnp_lib = rnp_prefix.join("lib");
    let rnp_inc = rnp_prefix.join("include");
    println!("cargo:lib_dir={}", rnp_lib.display());
    println!("cargo:include_dir={}", rnp_inc.display());
    println!("cargo:botan_lib_dir={}", botan_lib_dir.display());
}

// --- Utility functions ---

fn nproc() -> String {
    Command::new("nproc")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "4".to_string())
}

fn download_and_extract(url: &str, dest: &PathBuf) {
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
    let _ = std::fs::remove_file(&tarball);
}

fn run(cmd: &mut Command, label: &str) {
    let status = cmd
        .status()
        .unwrap_or_else(|e| panic!("rnp-src: failed to run {label}: {e}"));
    assert!(
        status.success(),
        "rnp-src: {label} failed with status {status}"
    );
}

// --- Per-dependency build functions ---

fn build_jsonc(src_dir: &PathBuf, prefix: &PathBuf) {
    let jsonc_src = src_dir.join(format!("json-c-{JSON_C_VERSION}"));
    if !jsonc_src.exists() {
        let url = format!(
            "https://s3.amazonaws.com/json-c_releases/releases/json-c-{JSON_C_VERSION}.tar.gz"
        );
        download_and_extract(&url, src_dir);
    }

    let build_dir = src_dir.join("json-c-build");
    run(
        Command::new("cmake")
            .args([
                "-S",
                jsonc_src.to_str().unwrap(),
                "-B",
                build_dir.to_str().unwrap(),
            ])
            .args(["-DCMAKE_BUILD_TYPE=Release"])
            .args(["-DBUILD_SHARED_LIBS=OFF", "-DBUILD_TESTING=OFF"])
            .arg(format!("-DCMAKE_INSTALL_PREFIX={}", prefix.display()))
            .arg("-DCMAKE_POLICY_VERSION_MINIMUM=3.5"),
        "json-c cmake",
    );
    run(
        Command::new("cmake").args([
            "--build",
            build_dir.to_str().unwrap(),
            "--parallel",
            &nproc(),
        ]),
        "json-c build",
    );
    run(
        Command::new("cmake").args(["--install", build_dir.to_str().unwrap()]),
        "json-c install",
    );
}

fn build_zlib(src_dir: &PathBuf, prefix: &PathBuf) {
    let zlib_src = src_dir.join(format!("zlib-{ZLIB_VERSION}"));
    if !zlib_src.exists() {
        let url = format!(
            "https://github.com/madler/zlib/releases/download/v{ZLIB_VERSION}/zlib-{ZLIB_VERSION}.tar.gz"
        );
        download_and_extract(&url, src_dir);
    }

    let build_dir = src_dir.join("zlib-build");
    run(
        Command::new("cmake")
            .args([
                "-S",
                zlib_src.to_str().unwrap(),
                "-B",
                build_dir.to_str().unwrap(),
            ])
            .args(["-DCMAKE_BUILD_TYPE=Release", "-DBUILD_SHARED_LIBS=OFF"])
            .arg(format!("-DCMAKE_INSTALL_PREFIX={}", prefix.display())),
        "zlib cmake",
    );
    run(
        Command::new("cmake").args([
            "--build",
            build_dir.to_str().unwrap(),
            "--parallel",
            &nproc(),
        ]),
        "zlib build",
    );
    run(
        Command::new("cmake").args(["--install", build_dir.to_str().unwrap()]),
        "zlib install",
    );
}

fn build_bzip2(src_dir: &PathBuf, prefix: &PathBuf) {
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
    std::fs::write(
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

    std::fs::create_dir_all(prefix.join("lib")).ok();
    std::fs::create_dir_all(prefix.join("include")).ok();
    std::fs::copy(
        bzip2_src.join("libbz2.a"),
        prefix.join("lib").join("libbz2.a"),
    )
    .unwrap();
    std::fs::copy(
        bzip2_src.join("bzlib.h"),
        prefix.join("include").join("bzlib.h"),
    )
    .unwrap();
}

fn build_librnp(
    src_dir: &PathBuf,
    prefix: &PathBuf,
    botan_prefix: &PathBuf,
    jsonc_prefix: &PathBuf,
    zlib_prefix: &PathBuf,
    bzip2_prefix: &PathBuf,
) {
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

    // Build the CMAKE_PREFIX_PATH from all dependency install prefixes.
    // On Unix, paths are separated by ';'. CMake accepts both ':' and ';'.
    let prefix_path = format!(
        "{};{};{};{}",
        botan_prefix.display(),
        jsonc_prefix.display(),
        zlib_prefix.display(),
        bzip2_prefix.display(),
    );

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
    .arg(format!("-DCMAKE_PREFIX_PATH={prefix_path}"))
    .arg(format!("-DCMAKE_INSTALL_PREFIX={}", prefix.display()))
    .arg("-DCMAKE_POLICY_VERSION_MINIMUM=3.5");

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
