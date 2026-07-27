// Build script: generate raw FFI bindings via bindgen and link against librnp.
//
// Three linking modes:
//
//   1. Default             — link the system-installed librnp (-lrnp).
//   2. --features vendored — hermetic static build: pinned release tarballs
//                            of librnp, Botan 3 and json-c are downloaded,
//                            sha256-verified, built from source and
//                            statically linked. No system crypto libraries
//                            are used or referenced. See vendor/README.md.
//   3. RNP_INCLUDE_DIR / RNP_LIB_DIR — explicit pointers to a non-system
//                            librnp install (e.g. a PQC-enabled build).
//
// Feature flags passed to bindgen when their respective Cargo feature is on:
//   --features pqc            -> -DRNP_EXPERIMENTAL_PQC
//   --features crypto-refresh -> -DRNP_EXPERIMENTAL_CRYPTO_REFRESH
// Each requires the linked librnp to have been built with the matching
// ENABLE_* CMake option ON (the vendored build does this automatically).

use std::{env, path::PathBuf};

fn main() {
    // ---------------------------------------------------------------------
    // 1. Locate the librnp headers and decide how to link.
    // ---------------------------------------------------------------------

    let (include_dir, lib_dirs, link_mode) = locate_librnp();

    let rnp_header = include_dir.join("rnp").join("rnp.h");
    if !rnp_header.exists() {
        panic!(
            "Could not find <rnp/rnp.h> under {}. Expected the header at {}.",
            include_dir.display(),
            rnp_header.display()
        );
    }

    for dir in &lib_dirs {
        println!("cargo:rustc-link-search=native={}", dir.display());
    }
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed={}", rnp_header.display());
    println!("cargo:rerun-if-env-changed=RNP_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=RNP_LIB_DIR");
    println!("cargo:rerun-if-env-changed=RNP_VENDOR_DIR");
    println!("cargo:rerun-if-env-changed=RNP_VENDOR_CMAKE_ARGS");

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
        LinkMode::HeadersOnly => {
            // Documentation build (docs.rs): headers only, nothing to link.
        }
        LinkMode::Vendored { static_lib_path } => {
            // Hermetic: everything librnp needs is linked statically —
            // librnp itself, libsexpp, Botan 3, json-c, zlib and bzip2.
            // All archives below except z/bz2 were built by the vendored
            // step. z/bz2 are static on Linux (libz.a / libbz2.a from the
            // zlib1g-dev / libbz2-dev packages); Apple ships no static
            // variants, so there they remain references to the OS-provided
            // libz/libbz2 — part of the base system, always present, and
            // not third-party crypto dependencies.
            println!("cargo:rustc-link-lib=static=rnp");
            println!("cargo:rustc-link-lib=static=sexpp");
            println!("cargo:rustc-link-lib=static=botan-3");
            println!("cargo:rustc-link-lib=static=json-c");
            if cfg!(target_os = "macos") {
                println!("cargo:rustc-link-lib=z");
                println!("cargo:rustc-link-lib=bz2");
            } else {
                // rustc verifies that static archives exist under an
                // explicit link-search path; system multiarch directories
                // are not probed implicitly.
                for dir in system_static_lib_dirs(&["z", "bz2"]) {
                    println!("cargo:rustc-link-search=native={}", dir.display());
                }
                println!("cargo:rustc-link-lib=static=z");
                println!("cargo:rustc-link-lib=static=bz2");
            }
            println!("cargo:rerun-if-changed={}", static_lib_path.display());
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

/// Locate the system directories holding `lib<name>.a` for each of `names`
/// (used by the vendored static link of zlib/bzip2 on Linux). Panics with a
/// package-manager hint when an archive is missing.
#[allow(dead_code)] // only used by the Vendored link arm
fn system_static_lib_dirs(names: &[&str]) -> Vec<PathBuf> {
    // Debian/Ubuntu multiarch triplet, e.g. aarch64-linux-gnu.
    let target = env::var("TARGET").unwrap_or_default();
    let mut parts = target.split('-');
    let arch = parts.next().unwrap_or_default();
    let abi = parts.next_back().unwrap_or_default();
    let multiarch = format!("{arch}-linux-{abi}");

    let candidates = [
        format!("/usr/lib/{multiarch}"),
        format!("/lib/{multiarch}"),
        "/usr/lib64".to_string(),
        "/usr/lib".to_string(),
        "/usr/local/lib".to_string(),
        "/lib".to_string(),
    ];

    let mut dirs: Vec<PathBuf> = Vec::new();
    for name in names {
        let file = format!("lib{name}.a");
        match candidates
            .iter()
            .map(PathBuf::from)
            .find(|d| d.join(&file).exists())
        {
            Some(dir) => {
                if !dirs.contains(&dir) {
                    dirs.push(dir);
                }
            }
            None => panic!(
                "rnp-rs: the vendored build links {file} statically but it was not \
                 found in the usual system locations. Install it — Debian/Ubuntu: \
                 `apt install zlib1g-dev libbz2-dev`; Fedora/RHEL: `dnf install \
                 zlib-static bzip2-static`; Alpine: `apk add zlib-static bzip2-static`."
            ),
        }
    }
    dirs
}

enum LinkMode {
    /// Default: `-lrnp` from a system search path (Homebrew, /usr/lib, ...).
    System,
    /// `RNP_LIB_DIR` (and optionally `RNP_INCLUDE_DIR`) point at a
    /// non-system install.
    Explicit,
    /// docs.rs documentation build: use the bundled headers so bindgen can
    /// run in the network-less sandbox; nothing is linked.
    HeadersOnly,
    /// `--features vendored`: hermetic static build of librnp + Botan 3 +
    /// json-c from pinned tarballs. The path is the resulting `librnp.a`.
    /// Only constructed when the `vendored` Cargo feature is on.
    #[allow(dead_code)]
    Vendored { static_lib_path: PathBuf },
}

fn locate_librnp() -> (PathBuf, Vec<PathBuf>, LinkMode) {
    // docs.rs builds documentation with all features enabled inside a
    // sandbox without network access. Use the headers bundled in the
    // published crate so bindgen succeeds; cargo doc links nothing.
    if env::var("DOCS_RS").is_ok() {
        let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
        let include_dir = manifest_dir
            .join("prebuilt")
            .join("x86_64-unknown-linux-gnu")
            .join("include");
        if include_dir.join("rnp").join("rnp.h").exists() {
            return (include_dir, Vec::new(), LinkMode::HeadersOnly);
        }
        // Fall through to the normal modes if the bundled headers are
        // absent (e.g. building from a git checkout without prebuilt/).
    }

    // Vendored has highest precedence.
    #[cfg(feature = "vendored")]
    {
        let vendored = vendored::build();
        let static_lib = vendored.librnp_a.clone();
        return (
            vendored.include_dir,
            vendored.lib_dirs,
            LinkMode::Vendored {
                static_lib_path: static_lib,
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
            let lib_dirs = lib_dir.into_iter().collect();
            return (include_dir, lib_dirs, LinkMode::Explicit);
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

        let lib_dirs = lib_dir.into_iter().collect();
        (include_dir, lib_dirs, LinkMode::System)
    }
}

// -----------------------------------------------------------------------
// Hermetic vendored build.
//
// Downloads the pinned release tarballs of librnp, Botan 3 and json-c
// (sha256-verified; or taken from RNP_VENDOR_DIR for air-gapped builds),
// builds them from source and statically links everything. The only build-
// time tools required are: a C/C++ compiler, cmake, make, tar, python3
// (Botan's configure script) and curl (skipped when RNP_VENDOR_DIR is
// set). On Linux the system libz.a / libbz2.a are linked statically, so
// zlib1g-dev / libbz2-dev are required there.
// -----------------------------------------------------------------------

#[cfg(feature = "vendored")]
mod vendored {
    use std::{
        env, fs,
        path::{Path, PathBuf},
        process::Command,
    };

    /// A pinned upstream source tarball.
    struct VendorCrate {
        /// Human-readable name for progress and error messages.
        name: &'static str,
        /// Pinned upstream version.
        version: &'static str,
        /// Tarball file name; also the name expected inside RNP_VENDOR_DIR.
        file: &'static str,
        /// Download URL used when RNP_VENDOR_DIR is not set.
        url: &'static str,
        /// Expected lowercase hex SHA-256 of the tarball.
        sha256: &'static str,
        /// Top-level directory inside the tarball.
        top_dir: &'static str,
    }

    const LIBRNP: VendorCrate = VendorCrate {
        name: "librnp",
        version: "0.18.1",
        file: "rnp-v0.18.1.tar.gz",
        // Official release dist tarball (bundles the libsexpp submodule);
        // hash cross-checked against upstream's rnp-v0.18.1.sha256 asset.
        url: "https://github.com/rnpgp/rnp/releases/download/v0.18.1/rnp-v0.18.1.tar.gz",
        sha256: "423c8e32e1e591462f759adf8441b1c44bca96d9f5daff13b82e81a79f18ecfd",
        top_dir: "rnp-v0.18.1",
    };

    const BOTAN: VendorCrate = VendorCrate {
        name: "botan",
        version: "3.12.0",
        file: "botan-3.12.0.tar.gz",
        // GitHub source archive of the signed 3.12.0 tag (the .tar.xz on
        // botan.randombit.net holds identical sources but needs xz to
        // unpack; .tar.gz keeps the build dependency-free).
        url: "https://github.com/randombit/botan/archive/refs/tags/3.12.0.tar.gz",
        sha256: "cf152f47723876a7b8544925fc37183089933b95b9b30f41e79ab2f263ab7995",
        top_dir: "botan-3.12.0",
    };

    const JSON_C: VendorCrate = VendorCrate {
        name: "json-c",
        version: "0.18",
        file: "json-c-0.18.tar.gz",
        // Official json-c release tarball.
        url: "https://s3.amazonaws.com/json-c_releases/releases/json-c-0.18.tar.gz",
        sha256: "876ab046479166b869afc6896d288183bbc0e5843f141200c677b3e8dfb11724",
        top_dir: "json-c-0.18",
    };

    const VENDORS: &[&VendorCrate] = &[&LIBRNP, &BOTAN, &JSON_C];

    /// Botan 3 modules librnp needs — rnp upstream's `ci/botan3-modules`
    /// list with the Botan 3.8+ `curve25519` -> `x25519` module rename
    /// applied, plus the `pcurves` modules librnp hard-requires for Botan
    /// 3.7+ (PCURVES_IMPL, the five SECP curves, SM2 and Brainpool — see
    /// `_botan_required_features` in librnp's src/lib/CMakeLists.txt).
    /// Botan's configure pulls in module dependencies automatically.
    const BOTAN_MODULES: &[&str] = &[
        "aead",
        "aes",
        "auto_rng",
        "bigint",
        "blowfish",
        "camellia",
        "cast128",
        "cbc",
        "cfb",
        "crc24",
        "des",
        "dl_algo",
        "dl_group",
        "dsa",
        "eax",
        "ecc_key",
        "ecdh",
        "ecdsa",
        "ed25519",
        "elgamal",
        "eme_pkcs1",
        "emsa_pkcs1",
        "emsa_raw",
        "ffi",
        "hash",
        "raw_hash",
        "hmac",
        "hmac_drbg",
        "idea",
        "kdf",
        "md5",
        "ocb",
        "pgp_s2k",
        "rfc3394",
        "rmd160",
        "rsa",
        "sha1",
        "sha2_32",
        "sha2_64",
        "sha3",
        "sm2",
        "sm3",
        "sm4",
        "sp800_56a",
        "twofish",
        "x25519",
        // pcurves (required by librnp when Botan >= 3.7). pcurves_impl is an
        // internal module and cannot be named here; every per-curve module
        // pulls it in via its own `requires`.
        "pcurves_secp192r1",
        "pcurves_secp256k1",
        "pcurves_secp256r1",
        "pcurves_secp384r1",
        "pcurves_secp521r1",
        "pcurves_sm2p256v1",
        "pcurves_brainpool256r1",
        "pcurves_brainpool384r1",
        "pcurves_brainpool512r1",
    ];

    /// Additional Botan modules for librnp's ENABLE_PQC=ON (from rnp
    /// upstream's `ci/botan3-pqc-modules`).
    const BOTAN_MODULES_PQC: &[&str] = &[
        "dilithium",
        "kyber",
        "sphincsplus_sha2",
        "sphincsplus_shake",
        "kmac",
    ];

    /// Additional Botan modules for librnp's ENABLE_CRYPTO_REFRESH=ON.
    const BOTAN_MODULES_CRYPTO_REFRESH: &[&str] = &["hkdf"];

    /// Bump when the build recipe changes; part of the rebuild stamp.
    const RECIPE_VERSION: u32 = 1;

    /// Locations the rest of the build script needs after a vendored build.
    pub struct VendoredLayout {
        /// Directory containing `rnp/rnp.h`.
        pub include_dir: PathBuf,
        /// Directories to add to the native library search path.
        pub lib_dirs: Vec<PathBuf>,
        /// The built `librnp.a` (used for rerun-if-changed).
        pub librnp_a: PathBuf,
    }

    pub fn build() -> VendoredLayout {
        let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
        let target = env::var("TARGET").unwrap_or_default();
        let host = env::var("HOST").unwrap_or_default();
        if target != host {
            panic!(
                "rnp-rs: the vendored feature does not support cross-compilation \
                 (host {host}, target {target}). Build librnp + Botan + json-c \
                 externally and point RNP_INCLUDE_DIR / RNP_LIB_DIR at the \
                 install instead."
            );
        }
        if env::var_os("RNP_VENDOR_BACKEND").is_some() {
            println!(
                "cargo:warning=rnp-rs: RNP_VENDOR_BACKEND is ignored — the hermetic \
                 vendored build always uses the pinned Botan 3 backend."
            );
        }

        let pqc = cfg!(feature = "pqc");
        let crypto_refresh = cfg!(feature = "crypto-refresh");

        let root = out_dir.join("vendor");
        let rnp_install = root.join("rnp-install");
        let botan_install = root.join("botan-install");
        let jsonc_install = root.join("jsonc-install");

        let layout = VendoredLayout {
            include_dir: rnp_install.join("include"),
            lib_dirs: vec![
                rnp_install.join("lib"),
                botan_install.join("lib"),
                jsonc_install.join("lib"),
            ],
            librnp_a: rnp_install.join("lib").join("librnp.a"),
        };

        let stamp = format!(
            "recipe={RECIPE_VERSION} target={target} pqc={pqc} \
             crypto_refresh={crypto_refresh} librnp={} botan={} json-c={}",
            LIBRNP.version, BOTAN.version, JSON_C.version
        );
        let stamp_path = root.join("VENDORED-STAMP");

        let artifacts = [
            layout.librnp_a.clone(),
            rnp_install.join("lib").join("libsexpp.a"),
            rnp_install.join("include").join("rnp").join("rnp.h"),
            botan_install.join("lib").join("libbotan-3.a"),
            jsonc_install.join("lib").join("libjson-c.a"),
        ];

        let up_to_date = fs::read_to_string(&stamp_path)
            .map(|s| s == stamp)
            .unwrap_or(false)
            && artifacts.iter().all(|p| p.exists());
        if up_to_date {
            return layout;
        }

        // Full rebuild from a clean slate so a previous partial failure can
        // never leave a mixed state behind.
        if root.exists() {
            fs::remove_dir_all(&root).expect("failed to clean the vendored build dir");
        }
        let dist = root.join("dist");
        let src = root.join("src");
        fs::create_dir_all(&dist)
            .and_then(|()| fs::create_dir_all(&src))
            .expect("failed to create the vendored build dirs");

        // 1. Obtain, sha256-verify and extract the pinned tarballs.
        for v in VENDORS {
            let tarball = obtain(v, &dist);
            verify_sha256(v, &tarball);
            extract(v, &tarball, &src);
        }

        // 2. Botan: its own configure.py + make, static library only.
        build_botan(
            &src.join(BOTAN.top_dir),
            &botan_install,
            pqc,
            crypto_refresh,
        );

        // 3. json-c: CMake, static library only.
        build_json_c(&src.join(JSON_C.top_dir), &jsonc_install);

        // 4. librnp: CMake against the two installs above (libsexpp is
        //    bundled in librnp's dist tarball).
        build_librnp(
            &src.join(LIBRNP.top_dir),
            &rnp_install,
            &botan_install,
            &jsonc_install,
            pqc,
            crypto_refresh,
        );

        for artifact in &artifacts {
            assert!(
                artifact.exists(),
                "rnp-rs: vendored build finished but {} is missing — inspect the \
                 build output above.",
                artifact.display()
            );
        }
        fs::write(&stamp_path, stamp).expect("failed to write the vendored stamp");
        println!(
            "cargo:warning=rnp-rs: hermetic vendored build complete — librnp {}, \
             Botan {}, json-c {} (all static)",
            LIBRNP.version, BOTAN.version, JSON_C.version
        );
        layout
    }

    /// Place the tarball for `v` at `dist/<file>`, either by copying it
    /// from RNP_VENDOR_DIR (air-gapped) or by downloading it with curl.
    fn obtain(v: &VendorCrate, dist: &Path) -> PathBuf {
        let dest = dist.join(v.file);
        if let Ok(dir) = env::var("RNP_VENDOR_DIR") {
            let src = Path::new(&dir).join(v.file);
            if !src.exists() {
                panic!(
                    "rnp-rs: RNP_VENDOR_DIR is set to `{dir}` but {} is not there. \
                     Expected files: {}",
                    v.file,
                    VENDORS
                        .iter()
                        .map(|v| v.file)
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            println!(
                "cargo:warning=rnp-rs: using {} {} from RNP_VENDOR_DIR",
                v.name, v.version
            );
            fs::copy(&src, &dest)
                .unwrap_or_else(|e| panic!("rnp-rs: failed to copy {}: {e}", src.display()));
        } else {
            println!(
                "cargo:warning=rnp-rs: downloading {} {} from {}",
                v.name, v.version, v.url
            );
            let tmp = dist.join(format!("{}.part", v.file));
            let status = Command::new("curl")
                .args(["-fSL", "--retry", "3", "--connect-timeout", "30", "-o"])
                .arg(&tmp)
                .arg(v.url)
                .status();
            match status {
                Ok(s) if s.success() => {}
                Ok(s) => panic!(
                    "rnp-rs: curl failed to download {} (exit status {s}). For offline \
                     builds, place the pinned tarballs in a directory and set \
                     RNP_VENDOR_DIR to it.",
                    v.url
                ),
                Err(e) => panic!(
                    "rnp-rs: failed to run curl ({e}). Install curl, or place the pinned \
                     tarballs in a directory and set RNP_VENDOR_DIR to it (expected \
                     files: {}).",
                    VENDORS
                        .iter()
                        .map(|v| v.file)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            }
            fs::rename(&tmp, &dest).expect("failed to move the downloaded tarball into place");
        }
        dest
    }

    /// Verify the tarball against the pinned SHA-256, or abort the build.
    fn verify_sha256(v: &VendorCrate, tarball: &Path) {
        let actual = sha256::file_hex(tarball);
        assert!(
            actual == v.sha256,
            "rnp-rs: SHA-256 mismatch for {}\n  expected: {}\n  actual:   {}\nThe pinned \
             {} tarball failed verification. If you supplied it via RNP_VENDOR_DIR, \
             replace it with the genuine {} release archive.",
            v.file,
            v.sha256,
            actual,
            v.name,
            v.version
        );
    }

    /// Unpack the tarball into `src` and check the expected top-level
    /// directory appeared.
    fn extract(v: &VendorCrate, tarball: &Path, src: &Path) {
        run(
            Command::new("tar")
                .arg("-xzf")
                .arg(tarball)
                .arg("-C")
                .arg(src),
            "tar extract",
        );
        let top = src.join(v.top_dir);
        assert!(
            top.is_dir(),
            "rnp-rs: extracted {} but the expected directory {} did not appear",
            v.file,
            top.display()
        );
    }

    /// Build Botan as a static library with the minimized module set.
    fn build_botan(src_dir: &Path, prefix: &Path, pqc: bool, crypto_refresh: bool) {
        println!(
            "cargo:warning=rnp-rs: building Botan {} (static, minimized module set) — \
             this takes a few minutes",
            BOTAN.version
        );
        let python = find_python();
        let mut modules: Vec<&str> = BOTAN_MODULES.to_vec();
        if pqc {
            modules.extend(BOTAN_MODULES_PQC);
        }
        if crypto_refresh {
            modules.extend(BOTAN_MODULES_CRYPTO_REFRESH);
        }
        run(
            Command::new(python)
                .arg("configure.py")
                .arg(format!("--prefix={}", prefix.display()))
                .arg("--minimized-build")
                .arg(format!("--enable-modules={}", modules.join(",")))
                .arg("--without-documentation")
                .arg("--build-targets=static")
                // The archive is linked into Rust binaries, which are PIE
                // on most targets.
                .arg("--extra-cxxflags=-fPIC")
                .current_dir(src_dir),
            "Botan configure.py",
        );
        let jobs = env::var("NUM_JOBS").unwrap_or_else(|_| "4".to_string());
        run(
            Command::new("make")
                .arg(format!("-j{jobs}"))
                .arg("install")
                .current_dir(src_dir),
            "Botan make install",
        );
    }

    /// Build json-c as a static library.
    fn build_json_c(src_dir: &Path, prefix: &Path) {
        println!(
            "cargo:warning=rnp-rs: building json-c {} (static)",
            JSON_C.version
        );
        let mut config = cmake::Config::new(src_dir);
        config
            .out_dir(prefix)
            .define("BUILD_SHARED_LIBS", "OFF")
            .define("BUILD_STATIC_LIBS", "ON")
            .define("BUILD_TESTING", "OFF")
            // apps/ carries a pre-3.5 cmake_minimum_required that CMake 4
            // rejects; the apps are not needed, only the library is.
            .define("BUILD_APPS", "OFF")
            .define("DISABLE_WERROR", "ON")
            .define("CMAKE_BUILD_TYPE", "Release")
            .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON");
        config.build();
    }

    /// Build librnp statically against the pinned Botan and json-c.
    fn build_librnp(
        src_dir: &Path,
        prefix: &Path,
        botan_prefix: &Path,
        jsonc_prefix: &Path,
        pqc: bool,
        crypto_refresh: bool,
    ) {
        println!(
            "cargo:warning=rnp-rs: building librnp {} (static)",
            LIBRNP.version
        );
        let mut config = cmake::Config::new(src_dir);
        config
            .out_dir(prefix)
            .define("CRYPTO_BACKEND", "botan3")
            .define("BUILD_SHARED_LIBS", "OFF")
            .define("BUILD_TESTING", "OFF")
            .define("ENABLE_DOC", "OFF")
            .define("ENABLE_PQC", if pqc { "ON" } else { "OFF" })
            .define(
                "ENABLE_CRYPTO_REFRESH",
                if crypto_refresh { "ON" } else { "OFF" },
            )
            .define("CMAKE_BUILD_TYPE", "Release")
            .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON")
            // librnp's FindBotan.cmake restricts the search to this prefix
            // (NO_DEFAULT_PATH), so a system-installed Botan is never used.
            .define("BOTAN_ROOT_DIR", botan_prefix)
            // CMAKE_PREFIX_PATH is searched before pkg-config results and
            // system paths, so the pinned json-c always wins.
            .define("CMAKE_PREFIX_PATH", jsonc_prefix)
            // librnp 0.18.1's mem.cpp uses strlen without #include <cstring>;
            // GCC 13+ rejects that (clang tolerates it). Force the include
            // in every compilation unit instead of patching the tarball.
            .cxxflag("-include cstring");

        // Allow consumers to pass through extra CMake args.
        if let Ok(extra) = env::var("RNP_VENDOR_CMAKE_ARGS") {
            for arg in extra.split_whitespace() {
                if let Some((k, v)) = arg.split_once('=') {
                    config.define(k, v);
                }
            }
        }

        config.build();
    }

    /// Locate a Python 3 interpreter for Botan's configure script.
    fn find_python() -> String {
        let mut candidates: Vec<String> = env::var("PYTHON").into_iter().collect();
        candidates.push("python3".to_string());
        candidates.push("python".to_string());
        for cand in &candidates {
            let ok = Command::new(cand)
                .arg("--version")
                .output()
                .map(|o| {
                    o.status.success()
                        && (String::from_utf8_lossy(&o.stdout).starts_with("Python 3")
                            || String::from_utf8_lossy(&o.stderr).starts_with("Python 3"))
                })
                .unwrap_or(false);
            if ok {
                return cand.clone();
            }
        }
        panic!(
            "rnp-rs: python3 is required to build the vendored Botan (its build is \
             driven by configure.py). Install python3 or set PYTHON to a Python 3 \
             interpreter."
        );
    }

    fn run(cmd: &mut Command, what: &str) {
        let status = cmd
            .status()
            .unwrap_or_else(|e| panic!("rnp-rs: failed to run {what}: {e}"));
        assert!(
            status.success(),
            "rnp-rs: {what} failed with {status} — inspect the build output above."
        );
    }

    // -------------------------------------------------------------------
    // Minimal SHA-256 implementation (used to verify the pinned tarballs;
    // avoids any external tool or crate dependency).
    // -------------------------------------------------------------------

    mod sha256 {
        use std::{fs::File, io::Read, path::Path};

        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];

        pub fn file_hex(path: &Path) -> String {
            let mut file = File::open(path)
                .unwrap_or_else(|e| panic!("rnp-rs: cannot open {}: {e}", path.display()));
            let mut state: [u32; 8] = [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ];
            let mut total: u64 = 0;
            let mut pending: Vec<u8> = Vec::with_capacity(64);
            let mut buf = [0u8; 64 * 1024];

            loop {
                let n = file
                    .read(&mut buf)
                    .unwrap_or_else(|e| panic!("rnp-rs: cannot read {}: {e}", path.display()));
                if n == 0 {
                    break;
                }
                total += n as u64;
                pending.extend_from_slice(&buf[..n]);
                let mut chunks = pending.chunks_exact(64);
                for chunk in &mut chunks {
                    compress(&mut state, chunk);
                }
                let rest = chunks.remainder().to_vec();
                pending = rest;
            }

            // Padding: 0x80, zeros, then the 64-bit big-endian bit length.
            let bit_len = total * 8;
            pending.push(0x80);
            while pending.len() % 64 != 56 {
                pending.push(0);
            }
            pending.extend_from_slice(&bit_len.to_be_bytes());
            for chunk in pending.chunks_exact(64) {
                compress(&mut state, chunk);
            }

            state.iter().map(|w| format!("{w:08x}")).collect()
        }

        fn compress(state: &mut [u32; 8], block: &[u8]) {
            debug_assert_eq!(block.len(), 64);
            let mut w = [0u32; 64];
            for (i, word) in block.chunks_exact(4).enumerate() {
                w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
            }
            for i in 16..64 {
                let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
                let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
                w[i] = w[i - 16]
                    .wrapping_add(s0)
                    .wrapping_add(w[i - 7])
                    .wrapping_add(s1);
            }

            let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
            for i in 0..64 {
                let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
                let ch = (e & f) ^ ((!e) & g);
                let t1 = h
                    .wrapping_add(s1)
                    .wrapping_add(ch)
                    .wrapping_add(K[i])
                    .wrapping_add(w[i]);
                let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
                let maj = (a & b) ^ (a & c) ^ (b & c);
                let t2 = s0.wrapping_add(maj);
                h = g;
                g = f;
                f = e;
                e = d.wrapping_add(t1);
                d = c;
                c = b;
                b = a;
                a = t1.wrapping_add(t2);
            }

            state[0] = state[0].wrapping_add(a);
            state[1] = state[1].wrapping_add(b);
            state[2] = state[2].wrapping_add(c);
            state[3] = state[3].wrapping_add(d);
            state[4] = state[4].wrapping_add(e);
            state[5] = state[5].wrapping_add(f);
            state[6] = state[6].wrapping_add(g);
            state[7] = state[7].wrapping_add(h);
        }
    }
}
