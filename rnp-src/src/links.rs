//! Shared types and constants between `rnp-src`'s build.rs and its lib.rs.
//!
//! Both files path-include this module so the links contract has a single
//! source of truth and the pure logic is unit-testable from lib.rs.

use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------
// links contract — single source of truth for the cargo:foo=… ↔
// DEP_RNP_FOO env-var convention used between this crate's build.rs and
// rnp-rs's build.rs.
// ---------------------------------------------------------------------

/// The `links = "rnp"` Cargo prefix that downstream build scripts see
/// on env vars emitted by this crate.
pub const LINKS_ENV_PREFIX: &str = "DEP_RNP_";

/// Suffix appended to a dependency name to form its lib-dir env var.
/// `botan` → `DEP_RNP_BOTAN_LIB_DIR`.
pub const LINKS_LIB_DIR_SUFFIX: &str = "_LIB_DIR";

/// All dependency names whose `<prefix>/lib` rnp-src emits via
/// `cargo:<name>_lib_dir=<path>`. Adding a dep here makes both the
/// emitter (`Deps::emit_cargo_paths`) and the consumer (`rnp-rs/build.rs`)
/// pick it up automatically.
pub const DEPS: &[&str] = &["botan", "jsonc", "zlib", "bzip2"];

/// Compose the env-var name rnp-rs should read for a given dep's lib dir.
///
/// Cargo uppercases the entire env var name when surfacing `cargo:key=value`
/// directives as `DEP_<LINKS>_<KEY>` env vars, so we uppercase too to match.
pub fn lib_dir_env_var(name: &str) -> String {
    format!("{LINKS_ENV_PREFIX}{name}{LINKS_LIB_DIR_SUFFIX}").to_uppercase()
}

// ---------------------------------------------------------------------
// Deps — collection of per-dep install prefixes.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct Deps {
    items: Vec<Dep>,
}

#[derive(Debug, Clone)]
pub struct Dep {
    pub name: &'static str,
    pub prefix: PathBuf,
}

impl Deps {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn push(&mut self, name: &'static str, prefix: PathBuf) {
        assert!(
            DEPS.contains(&name),
            "rnp-src: Deps::push({name}) — name not in DEPS; update src/links.rs first"
        );
        self.items.push(Dep { name, prefix });
    }

    pub fn by_name(&self, name: &str) -> Option<&Dep> {
        self.items.iter().find(|d| d.name == name)
    }

    /// Semicolon-joined list of dep prefixes, as cmake's CMAKE_PREFIX_PATH expects.
    pub fn cmake_prefix_path(&self) -> String {
        self.items
            .iter()
            .map(|d| d.prefix.display().to_string())
            .collect::<Vec<_>>()
            .join(";")
    }

    /// Emit `cargo:<name>_lib_dir=<prefix>/lib` lines so rnp-rs can find
    /// each dep's static library at link time.
    pub fn emit_cargo_paths(&self) {
        for dep in &self.items {
            println!(
                "cargo:{}_lib_dir={}",
                dep.name,
                dep.prefix.join("lib").display()
            );
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Dep> {
        self.items.iter()
    }
}

// ---------------------------------------------------------------------
// CmakeDep — config-driven description of a cmake-built tarball dep.
// ---------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CmakeDep {
    pub name: &'static str,
    pub version: &'static str,
    /// `format!`-style template; `{version}` is substituted at runtime.
    pub url_template: &'static str,
    /// Extra `-D` flags beyond the standard CMAKE_INSTALL_PREFIX /
    /// BUILD_SHARED_LIBS / CMAKE_BUILD_TYPE.
    pub extra_cmake_args: &'static [&'static str],
    /// cmake policy version floor; some old libs need <3.5.
    pub cmake_policy_minimum: Option<&'static str>,
}

impl CmakeDep {
    pub fn source_dir(&self, src_root: &Path) -> PathBuf {
        src_root.join(format!("{}-{}", self.name, self.version))
    }

    pub fn build_dir(&self, src_root: &Path) -> PathBuf {
        src_root.join(format!("{}-build", self.name))
    }

    pub fn url(&self) -> String {
        self.url_template.replace("{version}", self.version)
    }
}

/// Bundled cmake deps rnp-src knows how to build.
pub const JSON_C: CmakeDep = CmakeDep {
    name: "json-c",
    version: "0.17",
    url_template: "https://s3.amazonaws.com/json-c_releases/releases/json-c-{version}.tar.gz",
    extra_cmake_args: &["-DBUILD_TESTING=OFF"],
    cmake_policy_minimum: Some("3.5"),
};

pub const ZLIB: CmakeDep = CmakeDep {
    name: "zlib",
    version: "1.3.1",
    url_template: "https://github.com/madler/zlib/releases/download/v{version}/zlib-{version}.tar.gz",
    extra_cmake_args: &[],
    cmake_policy_minimum: None,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lib_dir_env_var_for_botan() {
        assert_eq!(lib_dir_env_var("botan"), "DEP_RNP_BOTAN_LIB_DIR");
    }

    #[test]
    fn lib_dir_env_var_for_zlib() {
        assert_eq!(lib_dir_env_var("zlib"), "DEP_RNP_ZLIB_LIB_DIR");
    }

    #[test]
    fn all_deps_produce_well_formed_env_vars() {
        for &name in DEPS {
            let var = lib_dir_env_var(name);
            assert!(var.starts_with(LINKS_ENV_PREFIX));
            assert!(var.ends_with(LINKS_LIB_DIR_SUFFIX));
        }
    }

    #[test]
    fn deps_cmake_prefix_path_joins_with_semicolon() {
        let mut deps = Deps::new();
        deps.push("botan", PathBuf::from("/a/botan"));
        deps.push("jsonc", PathBuf::from("/b/json-c"));
        assert_eq!(deps.cmake_prefix_path(), "/a/botan;/b/json-c");
    }

    #[test]
    fn empty_deps_yields_empty_prefix_path() {
        let deps = Deps::new();
        assert_eq!(deps.cmake_prefix_path(), "");
    }

    #[test]
    fn deps_by_name_finds_pushed_entry() {
        let mut deps = Deps::new();
        deps.push("botan", PathBuf::from("/x/botan"));
        assert_eq!(
            deps.by_name("botan").map(|d| d.prefix.clone()),
            Some(PathBuf::from("/x/botan"))
        );
        assert!(deps.by_name("jsonc").is_none());
    }

    #[test]
    fn deps_push_rejects_unknown_name() {
        let mut deps = Deps::new();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            deps.push("nonexistent", PathBuf::from("/x"));
        }));
        assert!(result.is_err(), "push should panic for unknown dep name");
    }

    #[test]
    fn cmake_dep_source_and_build_dirs() {
        let dep = CmakeDep {
            name: "json-c",
            version: "0.17",
            url_template: "",
            extra_cmake_args: &[],
            cmake_policy_minimum: None,
        };
        let root = Path::new("/tmp/src");
        assert_eq!(dep.source_dir(root), PathBuf::from("/tmp/src/json-c-0.17"));
        assert_eq!(dep.build_dir(root), PathBuf::from("/tmp/src/json-c-build"));
    }

    #[test]
    fn cmake_dep_url_substitutes_version() {
        assert_eq!(
            JSON_C.url(),
            "https://s3.amazonaws.com/json-c_releases/releases/json-c-0.17.tar.gz"
        );
        assert_eq!(
            ZLIB.url(),
            "https://github.com/madler/zlib/releases/download/v1.3.1/zlib-1.3.1.tar.gz"
        );
    }
}
