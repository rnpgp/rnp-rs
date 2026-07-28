// rnp-src build script: download + compile librnp and all deps from source.
//
// Execution order (guaranteed by Cargo):
//   1. botan-src is compiled (its build.rs builds Botan)
//   2. THIS build.rs runs (compiles json-c, zlib, bzip2, librnp)
//   3. rnp-rs's build.rs runs (calls rnp_src::lib_dir() for paths)

use std::env;
use std::path::PathBuf;
use std::process::Command;

const RNP_VERSION: &str = "0.18.1";
const JSON_C_VERSION: &str = "0.17";
const ZLIB_VERSION: &str = "1.3.1";
const BZIP2_VERSION: &str = "1.0.8";

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let src_dir = out_dir.join("src");
    let prefix = out_dir.join("install");
    std::fs::create_dir_all(&src_dir).ok();
    std::fs::create_dir_all(&prefix).ok();

    // Set macOS deployment target for consistent ABI compatibility.
    if cfg!(target_os = "macos") {
        env::set_var("MACOSX_DEPLOYMENT_TARGET", "11.0");
    }

    // --- 1. Botan (via botan-src crate) ---
    let botan_lib_dir = botan_src::lib_dir();
    let botan_include_dir = botan_lib_dir.join("..").join("include");
    eprintln!("rnp-src: botan lib_dir = {}", botan_lib_dir.display());

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
            &botan_lib_dir,
            &jsonc_prefix,
            &zlib_prefix,
            &bzip2_prefix,
        );
    }

    // Expose paths to rnp-src's lib.rs via env!()
    let rnp_lib = rnp_prefix.join("lib");
    let rnp_inc = rnp_prefix.join("include");
    println!("cargo:rustc-env=RNP_SRC_LIB_DIR={}", rnp_lib.display());
    println!("cargo:rustc-env=RNP_SRC_INCLUDE_DIR={}", rnp_inc.display());
    println!("cargo:rustc-env=RNP_SRC_BOTAN_LIB_DIR={}", botan_lib_dir.display());
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
    assert!(status.success(), "rnp-src: failed to extract tarball from {url}");
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
            .args(["-S", jsonc_src.to_str().unwrap(), "-B", build_dir.to_str().unwrap()])
            .args(["-DCMAKE_BUILD_TYPE=Release"])
            .args(["-DBUILD_SHARED_LIBS=OFF", "-DBUILD_TESTING=OFF"])
            .arg(format!("-DCMAKE_INSTALL_PREFIX={}", prefix.display()))
            .arg("-DCMAKE_POLICY_VERSION_MINIMUM=3.5"),
        "json-c cmake",
    );
    run(
        Command::new("cmake")
            .args(["--build", build_dir.to_str().unwrap(), "--parallel", &nproc()]),
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
            .args(["-S", zlib_src.to_str().unwrap(), "-B", build_dir.to_str().unwrap()])
            .args(["-DCMAKE_BUILD_TYPE=Release", "-DBUILD_SHARED_LIBS=OFF"])
            .arg(format!("-DCMAKE_INSTALL_PREFIX={}", prefix.display())),
        "zlib cmake",
    );
    run(
        Command::new("cmake")
            .args(["--build", build_dir.to_str().unwrap(), "--parallel", &nproc()]),
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
    std::fs::copy(bzip2_src.join("libbz2.a"), prefix.join("lib").join("libbz2.a")).unwrap();
    std::fs::copy(bzip2_src.join("bzlib.h"), prefix.join("include").join("bzlib.h")).unwrap();
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
        botan_prefix
            .parent()
            .unwrap_or(botan_prefix)
            .display(),
        jsonc_prefix.display(),
        zlib_prefix.display(),
        bzip2_prefix.display(),
    );

    let build_dir = src_dir.join("rnp-build");
    let mut cmd = Command::new("cmake");
    cmd.args(["-S", rnp_src.to_str().unwrap(), "-B", build_dir.to_str().unwrap()])
        .args([format!("-DCMAKE_C_COMPILER={cc}"), format!("-DCMAKE_CXX_COMPILER={cxx}")])
        .args(["-DCRYPTO_BACKEND=botan3"])
        .args(["-DBUILD_SHARED_LIBS=OFF", "-DBUILD_TESTING=OFF", "-DENABLE_DOC=OFF"])
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
        Command::new("cmake")
            .args(["--build", build_dir.to_str().unwrap(), "--parallel", &nproc()]),
        "librnp build",
    );
    run(
        Command::new("cmake").args(["--install", build_dir.to_str().unwrap()]),
        "librnp install",
    );
}
