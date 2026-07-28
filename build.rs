// Build script: generate raw FFI bindings via bindgen and link against librnp.
//
// Two linking modes:
//
//   1. Default — link the system-installed librnp (-lrnp).
//   2. --features vendored — download + compile librnp and all deps
//      (Botan, json-c, zlib, bzip2) from source at build time.
//      Fully self-contained: no system librnp/Botan/json-c needed.
//
// Feature flags passed to bindgen when their respective Cargo feature is on:
//   --features pqc            -> -DRNP_EXPERIMENTAL_PQC
//   --features crypto-refresh -> -DRNP_EXPERIMENTAL_CRYPTO_REFRESH

use std::{env, path::PathBuf, process::Command};

// -----------------------------------------------------------------------
// PQC conflict checks
// -----------------------------------------------------------------------

#[cfg(all(feature = "vendored-minimal", feature = "pqc"))]
compile_error!(
    "`vendored-minimal` and `pqc` are mutually exclusive — the minimal Botan \
     build drops the ML-KEM/ML-DSA/SLH-DSA modules. Use `vendored` when you need PQC."
);
#[cfg(all(feature = "vendored-openssl3", feature = "pqc"))]
compile_error!(
    "`vendored-openssl3` and `pqc` are mutually exclusive — OpenSSL 3.x does \
     not implement the PQC algorithms."
);

// -----------------------------------------------------------------------
// Constants for vendored source builds
// -----------------------------------------------------------------------

const RNP_VERSION: &str = "0.18.1";
const BOTAN_VERSION: &str = "3.12.0";
const JSON_C_VERSION: &str = "0.17";
const ZLIB_VERSION: &str = "1.3.1";
const BZIP2_VERSION: &str = "1.0.8";

fn nproc() -> String {
    Command::new("nproc")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "4".to_string())
}

/// Download a URL via curl and extract a .tar.gz into `dest`.
fn download_and_extract(url: &str, dest: &PathBuf) {
    let tarball = dest.join("download.tar.gz");
    let status = Command::new("curl")
        .args(["-sL", "-o"])
        .arg(&tarball)
        .arg(url)
        .status()
        .unwrap_or_else(|e| panic!("Failed to run curl: {e}"));
    assert!(status.success(), "Failed to download {url}");
    let status = Command::new("tar")
        .args(["xzf"])
        .arg(&tarball)
        .arg("-C")
        .arg(dest)
        .status()
        .unwrap_or_else(|e| panic!("Failed to run tar: {e}"));
    assert!(status.success(), "Failed to extract tarball from {url}");
    let _ = std::fs::remove_file(&tarball);
}

/// Run a command, panic on failure with context.
fn run(cmd: &mut Command, label: &str) {
    let status = cmd.status().unwrap_or_else(|e| panic!("Failed to run {label}: {e}"));
    assert!(status.success(), "{label} failed with status {status}");
}

// -----------------------------------------------------------------------
// Main
// -----------------------------------------------------------------------

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

    // -------------------------------------------------------------------
    // Generate bindings via bindgen.
    // -------------------------------------------------------------------

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

    // -------------------------------------------------------------------
    // Link against librnp.
    // -------------------------------------------------------------------

    match link_mode {
        LinkMode::System | LinkMode::Explicit => {
            println!("cargo:rustc-link-lib=dylib=rnp");
        }
        LinkMode::Vendored { .. } => {
            println!("cargo:rustc-link-lib=static=rnp");
            println!("cargo:rustc-link-lib=static=sexpp");
            // Botan is the only backend for the compile-from-source path.
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
    #[allow(dead_code)]
    Vendored { static_lib_path: PathBuf },
}

fn locate_librnp() -> (PathBuf, Option<PathBuf>, LinkMode) {
    #[cfg(feature = "vendored")]
    {
        let (include_dir, lib_dir, static_lib) = build_vendored();
        return (include_dir, Some(lib_dir), LinkMode::Vendored { static_lib_path: static_lib });
    }

    #[allow(unreachable_code)]
    {
        if let Ok(dir) = env::var("RNP_INCLUDE_DIR") {
            let include_dir = PathBuf::from(dir);
            let lib_dir = env::var("RNP_LIB_DIR").ok().map(PathBuf::from).or_else(|| {
                let mut candidate = include_dir.clone();
                candidate.pop();
                candidate.push("lib");
                if candidate.exists() { Some(candidate) } else { None }
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
            if candidate.exists() { Some(candidate) } else { None }
        } else {
            None
        };

        (include_dir, lib_dir, LinkMode::System)
    }
}

// -----------------------------------------------------------------------
// Vendored: download + compile from source.
// -----------------------------------------------------------------------

#[cfg(feature = "vendored")]
fn build_vendored() -> (PathBuf, PathBuf, PathBuf) {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let src_dir = out_dir.join("vendored-src");
    let prefix = out_dir.join("vendored-install");
    std::fs::create_dir_all(&src_dir).ok();
    std::fs::create_dir_all(&prefix).ok();

    // --- 1. Botan ---
    let botan_prefix = prefix.join("botan");
    let botan_lib = botan_prefix.join("lib").join("libbotan-3.a");
    if !botan_lib.exists() {
        eprintln!("rnp-rs vendored: building Botan {BOTAN_VERSION}...");
        build_botan(&src_dir, &botan_prefix);
    }

    // --- 2. json-c ---
    let jsonc_prefix = prefix.join("json-c");
    let jsonc_lib = jsonc_prefix.join("lib").join("libjson-c.a");
    if !jsonc_lib.exists() {
        eprintln!("rnp-rs vendored: building json-c {JSON_C_VERSION}...");
        build_jsonc(&src_dir, &jsonc_prefix);
    }

    // --- 3. zlib ---
    let zlib_prefix = prefix.join("zlib");
    let zlib_lib = zlib_prefix.join("lib").join("libz.a");
    if !zlib_lib.exists() {
        eprintln!("rnp-rs vendored: building zlib {ZLIB_VERSION}...");
        build_zlib(&src_dir, &zlib_prefix);
    }

    // --- 4. bzip2 ---
    let bzip2_prefix = prefix.join("bzip2");
    let bzip2_lib = bzip2_prefix.join("lib").join("libbz2.a");
    if !bzip2_lib.exists() {
        eprintln!("rnp-rs vendored: building bzip2 {BZIP2_VERSION}...");
        build_bzip2(&src_dir, &bzip2_prefix);
    }

    // --- 5. librnp ---
    let rnp_prefix = prefix.join("rnp");
    let rnp_lib = rnp_prefix.join("lib").join("librnp.a");
    if !rnp_lib.exists() {
        eprintln!("rnp-rs vendored: building librnp {RNP_VERSION}...");
        build_librnp(&src_dir, &rnp_prefix, &botan_prefix, &jsonc_prefix, &zlib_prefix, &bzip2_prefix);
    }

    (
        rnp_prefix.join("include"),
        rnp_prefix.join("lib"),
        rnp_lib,
    )
}

#[cfg(feature = "vendored")]
fn build_botan(src_dir: &PathBuf, prefix: &PathBuf) {
    let botan_src = src_dir.join(format!("Botan-{BOTAN_VERSION}"));
    if !botan_src.exists() {
        let url = format!("https://botan.randombit.net/releases/Botan-{BOTAN_VERSION}.tar.xz");
        let tarball = src_dir.join("botan.tar.xz");
        run(Command::new("curl").args(["-sL", "-o"]).arg(&tarball).arg(&url), "curl botan");
        run(Command::new("tar").args(["xf"]).arg(&tarball).arg("-C").arg(src_dir), "tar botan");
        let _ = std::fs::remove_file(&tarball);
    }

    // Detect compiler: clang on macOS, gcc elsewhere.
    let (cc, cc_bin) = if cfg!(target_os = "macos") {
        ("clang", "/usr/bin/clang++")
    } else {
        ("gcc", "g++")
    };

    let module_flag = if cfg!(feature = "vendored-minimal") {
        "--disable-modules=unsupported,tls,dtls,http_client,sockets,sqlite3,boost,pkcs11,tpm2,ml_kem,ml_dsa,slh_dsa_sha2,slh_dsa_shake,sm2,sm3,sm4,twofish,blowfish,cast_128,idea,ed448,x448,ripemd_160,brainpool256r1,brainpool384r1,brainpool512r1"
    } else {
        "--disable-modules=unsupported"
    };

    let build_dir = src_dir.join("botan-build");
    run(
        Command::new("python3")
            .arg("configure.py")
            .arg(format!("--prefix={}", prefix.display()))
            .arg(format!("--with-build-dir={}", build_dir.display()))
            .arg("--link-method=copy")
            .args(["--cc", cc, "--cc-bin", cc_bin])
            .arg("--disable-shared-library")
            .arg(module_flag)
            .current_dir(&botan_src),
        "botan configure",
    );

    let makefile = format!("{}/Makefile", build_dir.display());
    let jobs = nproc();
    run(
        Command::new("make").args(["-f", &makefile, "-j", &jobs]),
        "botan make",
    );
    run(
        Command::new("make").args(["-f", &makefile, "install"]),
        "botan install",
    );
}

#[cfg(feature = "vendored")]
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
        Command::new("cmake").args(["--build", build_dir.to_str().unwrap(), "--parallel", &nproc()]),
        "json-c build",
    );
    run(
        Command::new("cmake").args(["--install", build_dir.to_str().unwrap()]),
        "json-c install",
    );
}

#[cfg(feature = "vendored")]
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
        Command::new("cmake").args(["--build", build_dir.to_str().unwrap(), "--parallel", &nproc()]),
        "zlib build",
    );
    run(
        Command::new("cmake").args(["--install", build_dir.to_str().unwrap()]),
        "zlib install",
    );
}

#[cfg(feature = "vendored")]
fn build_bzip2(src_dir: &PathBuf, prefix: &PathBuf) {
    let bzip2_src = src_dir.join(format!("bzip2-{BZIP2_VERSION}"));
    if !bzip2_src.exists() {
        let url = format!("https://sourceware.org/pub/bzip2/bzip2-{BZIP2_VERSION}.tar.gz");
        download_and_extract(&url, src_dir);
    }

    let cc = if cfg!(target_os = "macos") { "/usr/bin/clang" } else { "gcc" };
    run(
        Command::new("make")
            .args(["libbz2.a"])
            .args(["-j", &nproc()])
            .args([format!("CC={cc}"), "CFLAGS=-O3 -fPIC".to_string()])
            .current_dir(&bzip2_src),
        "bzip2 make",
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

#[cfg(feature = "vendored")]
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

    let prefix_path = format!(
        "{};{};{};{}",
        botan_prefix.display(),
        jsonc_prefix.display(),
        zlib_prefix.display(),
        bzip2_prefix.display(),
    );

    let build_dir = src_dir.join("rnp-build");
    let pqc = if cfg!(feature = "pqc") { "ON" } else { "OFF" };
    let crypto_refresh = if cfg!(feature = "crypto-refresh") { "ON" } else { "OFF" };

    let mut cmd = Command::new("cmake");
    cmd.args(["-S", rnp_src.to_str().unwrap(), "-B", build_dir.to_str().unwrap()])
        .args([format!("-DCMAKE_C_COMPILER={cc}"), format!("-DCMAKE_CXX_COMPILER={cxx}")])
        .args(["-DCRYPTO_BACKEND=botan3"])
        .args(["-DBUILD_SHARED_LIBS=OFF", "-DBUILD_TESTING=OFF", "-DENABLE_DOC=OFF"])
        .args(["-DCMAKE_BUILD_TYPE=Release"])
        .arg(format!("-DCMAKE_CXX_FLAGS=-include cstring"))
        .arg(format!("-DCMAKE_PREFIX_PATH={prefix_path}"))
        .args([format!("-DENABLE_PQC={pqc}"), format!("-DENABLE_CRYPTO_REFRESH={crypto_refresh}")])
        .arg(format!("-DCMAKE_INSTALL_PREFIX={}", prefix.display()))
        .arg("-DCMAKE_POLICY_VERSION_MINIMUM=3.5");

    run(&mut cmd, "librnp cmake");
    run(
        Command::new("cmake").args(["--build", build_dir.to_str().unwrap(), "--parallel", &nproc()]),
        "librnp build",
    );
    run(
        Command::new("cmake").args(["--install", build_dir.to_str().unwrap()]),
        "librnp install",
    );
}
