//! The pre-flight gate against fixture repositories, and its parts.
//!
//! Fixture repositories live under `/tmp` directly, never
//! `std::env::temp_dir()`: under `cargo mutants` that follows `TMPDIR`
//! into the workspace's scratch copy.

use std::process::Command;

use super::index::{Calls, Function};
use super::*;

/// A scratch git repository, removed on drop.
struct Repo {
    root: PathBuf,
}

impl Repo {
    fn new(name: &str) -> Self {
        let root = Path::new("/tmp").join(format!(
            "hatchling-dev-preflight-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let repo = Self { root };
        repo.git(&["init", "-q"]);
        repo
    }

    fn write(&self, path: &str, text: &str) {
        let path = self.root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args([
                "-c",
                "user.name=fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(&self.root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    fn commit(&self) -> String {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", "test(fixture): snapshot"]);
        self.git(&["rev-parse", "HEAD"]).trim().to_owned()
    }

    fn findings(&self, base: &str) -> Vec<String> {
        run(&self.root, base)
            .unwrap()
            .into_iter()
            .map(|finding| format!("[{}] {}", finding.check, finding.message))
            .collect()
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

const INNER_ONLY: &str = r"pub fn inner(x: u32) -> u32 {
    x + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inner_adds_one() {
        assert_eq!(inner(1), 2);
    }
}
";

/// A one-line delegation tested only through its delegate, and a
/// comment that names it anyway.
const LYING: &str = r"pub fn checked(x: u32) -> u32 {
    inner(x)
}

pub fn inner(x: u32) -> u32 {
    x + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inner_adds_one() {
        // Mutants: `checked` replaced with 0 fails here.
        assert_eq!(inner(1), 2);
    }
}
";

/// The same change, honestly tested.
const HONEST: &str = r"pub fn checked(x: u32) -> u32 {
    inner(x)
}

pub fn inner(x: u32) -> u32 {
    x + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inner_adds_one() {
        // Mutants: `checked` replaced with 0, or `inner`'s `+` as `-`.
        assert_eq!(checked(1), 2);
        assert_eq!(inner(1), 2);
    }
}
";

#[test]
fn a_lying_comment_and_an_untested_delegation_fail_by_name() {
    let repo = Repo::new("lying");
    repo.write("crates/demo/src/lib.rs", INNER_ONLY);
    let base = repo.commit();
    repo.write("crates/demo/src/lib.rs", LYING);
    let expected = [
        "[mutant-comment] crates/demo/src/lib.rs:15: `// Mutants:` names `checked`, but `inner_adds_one` never calls it",
        "[delegation] crates/demo/src/lib.rs:1: `checked` changed, but no test calls it directly",
    ];
    assert_eq!(repo.findings(&base), expected);
    // Committed or not, the change is checked the same way.
    repo.commit();
    assert_eq!(repo.findings(&base), expected);
}

#[test]
fn a_true_comment_and_direct_calls_pass() {
    let repo = Repo::new("honest");
    repo.write("crates/demo/src/lib.rs", INNER_ONLY);
    let base = repo.commit();
    repo.write("crates/demo/src/lib.rs", HONEST);
    assert!(repo.findings(&base).is_empty());
    let head = repo.commit();
    assert!(repo.findings(&head).is_empty());
}

const CLAIMS_LIB: &str = r"pub fn alpha() {}
pub fn beta() {}
pub fn epsilon() {}

pub struct Gamma;

impl Gamma {
    pub fn delta() {
        // Mutants: `nothing` here: this is not a test.
    }
}
";

const CLAIMS_TEST: &str = r"use demo::*;

// Mutants: `alpha` above the test it belongs to,
// continued: `beta()` too, and `Gamma::delta`.
#[test]
fn names_its_targets() {
    alpha();
    beta();
    Gamma::delta();
}

// Mutants: `alpha`, but a blank line follows.

#[test]
fn detached() {
    alpha();
}

fn helper() {
    epsilon();
}

#[test]
fn through_a_helper() {
    // Mutants: `epsilon`, reached through `helper`;
    // `zeta` is not.
    helper();
}

#[test]
fn prose_only() {
    // Mutants: `>` to `>=`, `u32`, `true`, `Some`, `x.y`, unquoted alpha.
    alpha();
}
";

#[test]
fn claims_are_checked_where_they_sit_and_prose_is_not() {
    let repo = Repo::new("claims");
    repo.write("crates/demo/Cargo.toml", "[package]\nname = \"demo\"\n");
    let base = repo.commit();
    repo.write("crates/demo/src/lib.rs", CLAIMS_LIB);
    repo.write("crates/demo/tests/claims.rs", CLAIMS_TEST);
    assert_eq!(
        repo.findings(&base),
        [
            "[mutant-comment] crates/demo/tests/claims.rs:12: `// Mutants:` names `alpha`, but is neither in a test nor directly above one",
            "[mutant-comment] crates/demo/tests/claims.rs:26: `// Mutants:` names `zeta`, but `through_a_helper` never calls it",
        ]
    );
}

const HELPERS: &str = r"pub fn target() -> u32 {
    1
}

pub fn deep_target() -> u32 {
    2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check() -> u32 {
        target() + deeper()
    }

    fn deeper() -> u32 {
        deep_target()
    }

    #[test]
    fn via_helpers() {
        // Mutants: `target` and `deep_target`, two helpers down.
        assert_eq!(check(), 3);
    }

    #[test]
    fn unrelated_claim() {
        // Mutants: `unrelated`, `via_helpers`.
        assert_eq!(check(), 3);
    }
}
";

#[test]
fn test_helpers_in_a_test_module_count_as_the_test() {
    let repo = Repo::new("helpers");
    repo.write("crates/demo/Cargo.toml", "[package]\nname = \"demo\"\n");
    let base = repo.commit();
    repo.write("crates/demo/src/lib.rs", HELPERS);
    assert_eq!(
        repo.findings(&base),
        [
            "[mutant-comment] crates/demo/src/lib.rs:29: `// Mutants:` names `unrelated`, but `unrelated_claim` never calls it",
            "[mutant-comment] crates/demo/src/lib.rs:29: `// Mutants:` names `via_helpers`, but `unrelated_claim` never calls it",
        ]
    );
}

const DELEGATION_BASE: &str = r#"pub fn untested_unchanged() -> u32 {
    1
}

pub fn edited() -> u32 {
    2
}

pub fn shrunk() -> u32 {
    let a = 3;
    a
}

/// Doc.
pub fn doc_only() -> u32 {
    4
}

pub const VALUE: u32 = 5;

pub struct Thing;

impl Thing {
    pub fn new() -> Self {
        Self
    }

    pub fn get(&self) -> u32 {
        6
    }

    pub fn assoc_untested() -> u32 {
        7
    }
}

impl std::fmt::Display for Thing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "thing")
    }
}

pub fn from_elsewhere() -> u32 {
    8
}

pub fn in_a_macro() -> u32 {
    9
}

pub fn main() {}

#[cfg(test)]
mod tests;
"#;

const DELEGATION_CHANGED: &str = r#"pub fn untested_unchanged() -> u32 {
    1
}

pub fn edited() -> u32 {
    20
}

pub fn shrunk() -> u32 {
    a
}

/// Docs.
pub fn doc_only() -> u32 {
    4
}

pub const VALUE: u32 = 50;

pub struct Thing;

impl Thing {
    pub fn new() -> Self {
        Thing
    }

    pub fn get(&self) -> u32 {
        60
    }

    pub fn assoc_untested() -> u32 {
        70
    }
}

impl std::fmt::Display for Thing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Thing")
    }
}

pub fn from_elsewhere() -> u32 {
    80
}

pub fn in_a_macro() -> u32 {
    90
}

pub fn main() {
    let _ = 1;
}

#[cfg(test)]
mod tests;
"#;

const DELEGATION_TESTS: &str = r"use super::*;

#[test]
fn calls() {
    let thing = Thing::new();
    assert_eq!(thing.get(), 6);
    assert_eq!(in_a_macro(), 9);
}
";

#[test]
fn only_changed_functions_must_be_called_directly() {
    let repo = Repo::new("delegation");
    repo.write("crates/demo/src/lib.rs", DELEGATION_BASE);
    repo.write("crates/demo/src/tests.rs", DELEGATION_TESTS);
    repo.write(
        "crates/other/tests/uses.rs",
        "#[test]\nfn uses() {\n    assert_eq!(demo::from_elsewhere(), 8);\n}\n",
    );
    let base = repo.commit();
    repo.write("crates/demo/src/lib.rs", DELEGATION_CHANGED);
    // A changed test file is not itself checked.
    repo.write(
        "crates/demo/src/tests.rs",
        &format!("{DELEGATION_TESTS}\nfn unused_helper() {{}}\n"),
    );
    // New, untracked files: wholly changed.
    repo.write("crates/demo/src/extra.rs", "pub fn fresh() {}\n");
    repo.write(
        "crates/demo/src/main.rs",
        "fn main() {\n    demo::edited();\n}\n",
    );
    assert_eq!(
        repo.findings(&base),
        [
            "[delegation] crates/demo/src/extra.rs:1: `fresh` changed, but no test calls it directly",
            "[delegation] crates/demo/src/lib.rs:5: `edited` changed, but no test calls it directly",
            "[delegation] crates/demo/src/lib.rs:9: `shrunk` changed, but no test calls it directly",
            "[delegation] crates/demo/src/lib.rs:31: `Thing::assoc_untested` changed, but no test calls it directly",
            "[delegation] crates/demo/src/lib.rs:50: `main` changed, but no test calls it directly",
        ]
    );
}

#[test]
fn a_bad_base_or_an_unparsable_changed_file_is_an_error() {
    let repo = Repo::new("errors");
    repo.write("crates/demo/src/lib.rs", "pub fn fine() {}\n");
    repo.write("crates/demo/src/broken.rs", "fn (\n");
    let base = repo.commit();
    // An unchanged file that does not parse is not compiled code.
    assert!(repo.findings(&base).is_empty());
    for bad in ["", "-p", "--output=x"] {
        let error = run(&repo.root, bad).unwrap_err().to_string();
        assert!(error.contains("bad --base"), "{bad:?}: {error}");
    }
    let error = run(&repo.root, "no-such-commit").unwrap_err().to_string();
    assert!(
        error.contains("git") && error.contains("no-such-commit"),
        "{error}"
    );
    repo.write("crates/demo/src/broken.rs", "fn still (\n");
    let error = run(&repo.root, &base).unwrap_err().to_string();
    assert!(error.contains("broken.rs"), "{error}");
}

fn function(name: &str, start: usize, line: usize, end: usize) -> Function {
    Function {
        name: name.to_owned(),
        owner: None,
        implements: None,
        receiver: false,
        diverges: false,
        start,
        line,
        end,
        test: false,
        calls: Calls::default(),
    }
}

fn source(code: &str, test: bool) -> Source {
    Source {
        text: code.to_owned(),
        index: index::index(code).unwrap(),
        test,
    }
}

#[test]
fn a_change_touches_a_function_from_its_fn_line_to_its_brace() {
    let f = function("f", 8, 10, 20);
    let touches = |change: Change| touched(&f, &[change]);
    // The fn line and the closing brace are in; the doc lines above
    // and the line below are not.
    assert!(touches(Change::Lines {
        first: 10,
        last: 10
    }));
    assert!(touches(Change::Lines {
        first: 20,
        last: 25
    }));
    assert!(touches(Change::Lines { first: 1, last: 30 }));
    assert!(!touches(Change::Lines { first: 8, last: 9 }));
    assert!(!touches(Change::Lines {
        first: 21,
        last: 22
    }));
    // A deletion after the fn line or before the brace is in; one
    // before the fn line or after the brace is not.
    assert!(touches(Change::Deleted { after: 10 }));
    assert!(touches(Change::Deleted { after: 19 }));
    assert!(!touches(Change::Deleted { after: 9 }));
    assert!(!touches(Change::Deleted { after: 20 }));
    assert!(!touched(&f, &[]));
    assert!(touched(
        &f,
        &[
            Change::Lines { first: 1, last: 2 },
            Change::Deleted { after: 15 }
        ]
    ));
}

#[test]
fn a_diff_maps_each_file_to_its_new_side_hunks() {
    let diff = "diff --git a/crates/a/src/lib.rs b/crates/a/src/lib.rs
--- a/crates/a/src/lib.rs
+++ b/crates/a/src/lib.rs
@@ -3,2 +3,4 @@ fn f() {
+x
@@ -10 +12 @@
-y
+z
@@ -20,3 +21,0 @@
-gone
diff --git a/crates/a/src/old.rs b/crates/a/src/old.rs
--- a/crates/a/src/old.rs
+++ /dev/null
@@ -1,4 +0,0 @@
-a
--- /dev/null
+++ b/crates/b/src/new.rs
@@ -0,0 +1,2 @@
+n
";
    let changes = parse_diff(diff);
    assert_eq!(changes.len(), 2, "{changes:?}");
    assert_eq!(
        changes.get("crates/a/src/lib.rs").unwrap(),
        &[
            Change::Lines { first: 3, last: 6 },
            Change::Lines {
                first: 12,
                last: 12
            },
            Change::Deleted { after: 21 },
        ]
    );
    assert_eq!(
        changes.get("crates/b/src/new.rs").unwrap(),
        &[Change::Lines { first: 1, last: 2 }]
    );
}

#[test]
fn a_hunk_header_without_a_new_side_is_skipped() {
    for hunk in ["", "-1", "-1 2 @@", "-1 +x @@", "-1 +2,y @@", "-1 +,2 @@"] {
        assert_eq!(hunk_change(hunk), None, "{hunk:?}");
    }
    assert_eq!(
        hunk_change("-7,0 +8 @@"),
        Some(Change::Lines { first: 8, last: 8 })
    );
    assert_eq!(
        hunk_change("-7 +8,0 @@"),
        Some(Change::Deleted { after: 8 })
    );
    assert_eq!(
        hunk_change("-7 +8,3 @@ fn context()"),
        Some(Change::Lines { first: 8, last: 10 })
    );
}

#[test]
fn main_in_a_binary_and_syntax_trait_methods_are_exempt() {
    let none = BTreeSet::new();
    let declared: BTreeSet<String> = [String::from("crates/a/src/run_tool.rs")].into();
    let mut main = function("main", 1, 1, 2);
    assert!(exempt("crates/a/src/main.rs", &main, &none));
    assert!(exempt("crates/a/src/bin/tool.rs", &main, &none));
    assert!(!exempt("crates/a/src/lib.rs", &main, &none));
    assert!(!exempt("crates/a/src/domain.rs", &main, &none));
    // A `[[bin]]` target's own path, and only when it is declared.
    assert!(exempt("crates/a/src/run_tool.rs", &main, &declared));
    assert!(!exempt("crates/a/src/run_tool.rs", &main, &none));
    assert!(!exempt("crates/a/src/lib.rs", &main, &declared));
    let other = function("other", 1, 1, 2);
    assert!(!exempt("crates/a/src/main.rs", &other, &none));
    assert!(!exempt("crates/a/src/run_tool.rs", &other, &declared));
    main.owner = Some(String::from("Thing"));
    assert!(!exempt("crates/a/src/main.rs", &main, &none));
    assert!(!exempt("crates/a/src/run_tool.rs", &main, &declared));
    let mut fmt = function("fmt", 1, 1, 2);
    fmt.implements = Some(String::from("Display"));
    assert!(exempt("crates/a/src/lib.rs", &fmt, &none));
    fmt.implements = Some(String::from("Tool"));
    assert!(!exempt("crates/a/src/lib.rs", &fmt, &none));
}

#[test]
fn a_diverging_function_is_exempt_wherever_it_is() {
    let none = BTreeSet::new();
    let mut child = function("child_main", 1, 1, 2);
    assert!(!exempt("crates/a/src/cell.rs", &child, &none));
    child.diverges = true;
    assert!(exempt("crates/a/src/cell.rs", &child, &none));
    child.owner = Some(String::from("Cell"));
    assert!(exempt("crates/a/src/cell.rs", &child, &none));
}

#[test]
fn entry_points_come_from_each_crates_bin_paths() {
    let dir = Dir::new(
        "entry-points",
        &[
            (
                "crates/a/Cargo.toml",
                "[package]\nname = \"a\"\n\n[[bin]]\nname = \"tool\"\npath = \"src/run_tool.rs\"\n\n[[bin]]\nname = \"probe\"\npath = \"./src/extra/probe.rs\"\n\n[[bin]]\nname = \"conventional\"\n",
            ),
            ("crates/b/Cargo.toml", "[package]\nname = \"b\"\n"),
            ("crates/c/src/lib.rs", ""),
        ],
    );
    assert_eq!(
        entry_points(&dir.root).unwrap(),
        names(&["crates/a/src/extra/probe.rs", "crates/a/src/run_tool.rs"])
    );
    let broken = Dir::new(
        "entry-points-broken",
        &[("crates/d/Cargo.toml", "[[bin]\n")],
    );
    let error = entry_points(&broken.root).unwrap_err();
    assert!(error.to_string().contains("Cargo.toml"), "{error}");
    assert!(
        entry_points(&broken.root.join("crates")).is_err(),
        "no crates/ under it"
    );
}

#[test]
fn a_declared_binary_entry_point_is_exempt_but_its_helpers_are_not() {
    let repo = Repo::new("bin-path");
    let manifest =
        "[package]\nname = \"tool\"\n\n[[bin]]\nname = \"tool\"\npath = \"src/run_tool.rs\"\n";
    repo.write("crates/tool/Cargo.toml", manifest);
    repo.write(
        "crates/tool/src/run_tool.rs",
        "fn main() {\n    helper();\n}\n\nfn helper() {}\n",
    );
    let base = repo.commit();
    repo.write(
        "crates/tool/src/run_tool.rs",
        "fn main() {\n    helper();\n    helper();\n}\n\nfn helper() {\n    let _ = 1;\n}\n",
    );
    assert_eq!(
        repo.findings(&base),
        [
            "[delegation] crates/tool/src/run_tool.rs:6: `helper` changed, but no test calls it directly"
        ]
    );
}

#[test]
fn a_module_resolves_beside_a_root_or_under_its_parent() {
    let mut files = BTreeMap::new();
    for path in [
        "crates/a/src/lib.rs",
        "crates/a/src/one.rs",
        "crates/a/src/two/mod.rs",
        "crates/a/src/one/three.rs",
        "crates/a/src/two/four.rs",
        "crates/a/src/bin/tool.rs",
        "crates/a/src/bin/five.rs",
        "crates/a/tests/it.rs",
        "crates/a/tests/common/mod.rs",
    ] {
        files.insert(path.to_owned(), source("", false));
    }
    let resolve = |parent: &str, name: &str| module_file(&files, parent, name);
    let cases = [
        ("crates/a/src/lib.rs", "one", Some("crates/a/src/one.rs")),
        (
            "crates/a/src/lib.rs",
            "two",
            Some("crates/a/src/two/mod.rs"),
        ),
        (
            "crates/a/src/one.rs",
            "three",
            Some("crates/a/src/one/three.rs"),
        ),
        (
            "crates/a/src/two/mod.rs",
            "four",
            Some("crates/a/src/two/four.rs"),
        ),
        (
            "crates/a/src/bin/tool.rs",
            "five",
            Some("crates/a/src/bin/five.rs"),
        ),
        (
            "crates/a/tests/it.rs",
            "common",
            Some("crates/a/tests/common/mod.rs"),
        ),
        ("crates/a/src/lib.rs", "missing", None),
        ("crates/a/src/one.rs", "two", None),
        ("lib.rs", "one", None),
    ];
    for (parent, name, expected) in cases {
        assert_eq!(
            resolve(parent, name).as_deref(),
            expected,
            "{parent} {name}"
        );
    }
}

#[test]
fn test_modules_and_their_children_become_test_files() {
    let mut files = BTreeMap::new();
    for (path, text, test) in [
        (
            "crates/a/src/lib.rs",
            "#[cfg(test)]\nmod helpers;\nmod shipped;\n",
            false,
        ),
        ("crates/a/src/helpers.rs", "mod deeper;\n", false),
        ("crates/a/src/helpers/deeper.rs", "", false),
        ("crates/a/src/shipped.rs", "", false),
        ("crates/a/tests/it.rs", "mod common;\n", true),
        ("crates/a/tests/common/mod.rs", "", true),
    ] {
        files.insert(path.to_owned(), source(text, test));
    }
    mark_test_modules(&mut files);
    let marked: Vec<_> = files
        .iter()
        .map(|(path, file)| (path.as_str(), file.test))
        .collect();
    assert_eq!(
        marked,
        [
            ("crates/a/src/helpers.rs", true),
            ("crates/a/src/helpers/deeper.rs", true),
            ("crates/a/src/lib.rs", false),
            ("crates/a/src/shipped.rs", false),
            ("crates/a/tests/common/mod.rs", true),
            ("crates/a/tests/it.rs", true),
        ]
    );
}

#[test]
fn a_claim_is_a_quoted_lowercase_function_path() {
    for claim in [
        "name",
        "name()",
        "Type::name",
        "a::b::c_2",
        "Ab9::x_y",
        "_private",
        "v2",
        "snake_case",
        "Self::new",
    ] {
        assert!(is_function_path(claim), "{claim}");
    }
    for prose in [
        "",
        "Name",
        "CONST",
        "Type::Name",
        "u32",
        "true",
        "self",
        "x.y",
        ">=",
        "a::",
        "::a",
        "1abc",
        "a-b",
        "__",
        "_1",
        "name(x)",
        "a b",
        "Some",
    ] {
        assert!(!is_function_path(prose), "{prose}");
    }
}

#[test]
fn quoting_takes_matched_backtick_pairs_only() {
    fn all(text: &str) -> Vec<&str> {
        quoted(text).collect()
    }
    assert_eq!(all("a `b` c `d` e"), ["b", "d"]);
    assert_eq!(all("`a` `b` `c`"), ["a", "b", "c"]);
    assert_eq!(all("`b` `c"), ["b"]);
    assert_eq!(all("``"), [""]);
    assert!(all("no quotes").is_empty());
    assert!(all("`").is_empty());
}

#[test]
fn blocks_follow_the_pinned_form() {
    let text = "fn f() {
    // Mutants: `one` and `Two::two()`,
    // then `three`; `Four` is a type.
    code(); // Mutants: `trailing` is not a block
    //Mutants: `squashed` is prose
    // Mutants: `five`
    /// `doc` ends a block
    // `six` continues nothing
    // Mutants: `seven`
    //! `inner` ends it too
    let s = r#\"x
    // Mutants: `in_literal`
    // Mutants: `last_line`\"#;
}
// Mutants: `eight`";
    let claim = |line: usize, path: &str| Claim {
        line,
        path: path.to_owned(),
    };
    assert_eq!(
        blocks(text),
        [
            Block {
                start: 2,
                end: 3,
                claims: vec![claim(2, "one"), claim(2, "Two::two"), claim(3, "three")],
            },
            Block {
                start: 6,
                end: 6,
                claims: vec![claim(6, "five")],
            },
            Block {
                start: 9,
                end: 9,
                claims: vec![claim(9, "seven")],
            },
            Block {
                start: 15,
                end: 15,
                claims: vec![claim(15, "eight")],
            },
        ]
    );
}

#[test]
fn a_claim_names_its_last_segment() {
    let claim = |path: &str| Claim {
        line: 1,
        path: path.to_owned(),
    };
    assert_eq!(claim("Type::name").name(), "name");
    assert_eq!(claim("a::b::c").name(), "c");
    assert_eq!(claim("plain").name(), "plain");
}

#[test]
fn a_comment_belongs_to_the_function_it_is_in_or_directly_above() {
    let functions = [function("outer", 5, 6, 10), function("next", 12, 13, 15)];
    let owner = |start: usize, end: usize| {
        let block = Block {
            start,
            end,
            claims: Vec::new(),
        };
        attach(&functions, &block).map(|function| function.name.clone())
    };
    assert_eq!(owner(5, 6).as_deref(), Some("outer"));
    assert_eq!(owner(7, 10).as_deref(), Some("outer"));
    assert_eq!(owner(4, 4).as_deref(), Some("outer"));
    assert_eq!(owner(10, 11).as_deref(), Some("next"));
    assert_eq!(owner(3, 3), None);
    assert_eq!(owner(11, 12), None);
    assert_eq!(owner(16, 16), None);
}

#[test]
fn changes_and_git_read_the_repository() {
    let repo = Repo::new("changes");
    repo.write("crates/demo/src/lib.rs", "fn a() {}\nfn b() {}\n");
    let base = repo.commit();
    assert_eq!(
        git(&repo.root, &["rev-parse", "HEAD"]).unwrap().trim(),
        base
    );
    let error = git(&repo.root, &["rev-parse", "--verify", "no-such-ref"]).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("git rev-parse --verify no-such-ref"),
        "{error}"
    );
    repo.write("crates/demo/src/lib.rs", "fn a() {}\nfn b() { 1; }\n");
    repo.write("crates/demo/src/new.rs", "fn n() {}\n");
    repo.write("notes.txt", "untracked, not Rust: listed, never indexed\n");
    let changes = changes(&repo.root, &base).unwrap();
    let whole = vec![Change::Lines {
        first: 1,
        last: usize::MAX,
    }];
    let listed: Vec<_> = changes
        .iter()
        .map(|(path, change)| (path.as_str(), change.clone()))
        .collect();
    assert_eq!(
        listed,
        [
            (
                "crates/demo/src/lib.rs",
                vec![Change::Lines { first: 2, last: 2 }]
            ),
            ("crates/demo/src/new.rs", whole.clone()),
            ("notes.txt", whole),
        ]
    );
}

/// A plain fixture directory, no repository, removed on drop.
struct Dir {
    root: PathBuf,
}

impl Dir {
    fn new(name: &str, files: &[(&str, &str)]) -> Self {
        let root = Path::new("/tmp").join(format!(
            "hatchling-dev-preflight-dir-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        for (path, text) in files {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        Self { root }
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn names(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn the_tree_indexes_sources_and_tests_and_gathers_test_calls() {
    let dir = Dir::new(
        "tree",
        &[
            (
                "crates/a/src/lib.rs",
                "pub fn shipped() { inner(); }\n#[cfg(test)]\nmod support;\n",
            ),
            (
                "crates/a/src/support.rs",
                "fn helper() { from_support(); }\n",
            ),
            ("crates/a/src/nested/deep.rs", "fn deep() {}\n"),
            ("crates/a/src/notes.txt", "not rust\n"),
            ("crates/a/src/broken.rs", "fn (\n"),
            ("crates/a/tests/it.rs", "fn plain() { from_it(); }\n"),
            ("crates/a/benches/b.rs", "fn bench() { from_bench(); }\n"),
            ("crates/b/src/lib.rs", "#[test]\nfn t() { from_b(); }\n"),
        ],
    );
    let tree = Tree::load(&dir.root, &BTreeMap::new()).unwrap();
    let files: Vec<_> = tree
        .files
        .iter()
        .map(|(path, file)| (path.as_str(), file.test))
        .collect();
    assert_eq!(
        files,
        [
            ("crates/a/src/lib.rs", false),
            ("crates/a/src/nested/deep.rs", false),
            ("crates/a/src/support.rs", true),
            ("crates/a/tests/it.rs", true),
            ("crates/b/src/lib.rs", false),
        ]
    );
    assert_eq!(
        tree.test_calls().bare,
        names(&["from_b", "from_it", "from_support"])
    );
    // A changed file that does not parse cannot be checked.
    let mut changed = BTreeMap::new();
    changed.insert(String::from("crates/a/src/broken.rs"), Vec::new());
    let error = Tree::load(&dir.root, &changed)
        .err()
        .expect("a changed unparsable file must fail");
    assert!(error.to_string().contains("broken.rs"), "{error}");
}

#[test]
fn rust_files_and_collect_walk_only_sources_and_tests() {
    let dir = Dir::new(
        "walk",
        &[
            ("crates/a/src/lib.rs", ""),
            ("crates/a/src/x/y.rs", ""),
            ("crates/a/src/x/z.txt", ""),
            ("crates/a/tests/t.rs", ""),
            ("crates/a/examples/e.rs", ""),
            ("crates/b/build.rs", ""),
        ],
    );
    let mut found: Vec<_> = rust_files(&dir.root)
        .unwrap()
        .into_iter()
        .map(|path| {
            path.strip_prefix(&dir.root)
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    found.sort();
    assert_eq!(
        found,
        [
            "crates/a/src/lib.rs",
            "crates/a/src/x/y.rs",
            "crates/a/tests/t.rs"
        ]
    );
    let mut none = Vec::new();
    collect(&dir.root.join("missing"), &mut none).unwrap();
    assert!(none.is_empty());
    let mut some = Vec::new();
    collect(&dir.root.join("crates/a/src/x"), &mut some).unwrap();
    assert_eq!(some, [dir.root.join("crates/a/src/x/y.rs")]);
    assert!(
        rust_files(&dir.root.join("crates/a/src")).is_err(),
        "a root with no crates/ cannot be walked"
    );
}

#[test]
fn delegation_findings_name_untested_changed_functions_only() {
    let file = source(
        "fn tested() {}\nfn untested() {}\nimpl std::fmt::Display for X { fn fmt(&self) {} }\n#[test]\nfn t() { tested(); }\n",
        false,
    );
    let test_calls = file.index.functions.last().unwrap().calls.clone();
    let all = [Change::Lines {
        first: 1,
        last: usize::MAX,
    }];
    let none = BTreeSet::new();
    let findings: Vec<_> =
        delegation_findings("crates/a/src/lib.rs", &file, &all, &test_calls, &none)
            .into_iter()
            .map(|finding| (finding.check, finding.message))
            .collect();
    assert_eq!(
        findings,
        [(
            DELEGATION_CHECK,
            String::from(
                "crates/a/src/lib.rs:2: `untested` changed, but no test calls it directly"
            )
        )]
    );
    let first_only = [Change::Lines { first: 1, last: 1 }];
    assert!(
        delegation_findings(
            "crates/a/src/lib.rs",
            &file,
            &first_only,
            &test_calls,
            &none
        )
        .is_empty()
    );
}

#[test]
fn a_diverging_entry_point_is_skipped_but_what_it_calls_is_not() {
    let file = source(
        "fn child_main() -> ! {\n    if let Some(dir) = workdir() {}\n    loop {}\n}\nfn workdir() -> Option<u8> { None }\nfn decided() {}\n#[test]\nfn t() { decided(); }\n",
        false,
    );
    let test_calls = file.index.functions.last().unwrap().calls.clone();
    let all = [Change::Lines {
        first: 1,
        last: usize::MAX,
    }];
    let messages: Vec<String> = delegation_findings(
        "crates/a/src/cell.rs",
        &file,
        &all,
        &test_calls,
        &BTreeSet::new(),
    )
    .into_iter()
    .map(|finding| finding.message)
    .collect();
    assert_eq!(
        messages,
        ["crates/a/src/cell.rs:5: `workdir` changed, but no test calls it directly"]
    );
}

#[test]
fn comment_findings_check_claims_in_tests_only() {
    let text = "fn shipped() {\n    // Mutants: `ghost` in shipped code is prose.\n}\n#[test]\nfn t() {\n    // Mutants: `real`, `ghost`.\n    real();\n}\n// Mutants: `stray`\n";
    let messages = |test_file: bool| -> Vec<String> {
        comment_findings("crates/a/src/lib.rs", &source(text, test_file))
            .into_iter()
            .map(|finding| format!("[{}] {}", finding.check, finding.message))
            .collect()
    };
    let in_tests = [
        "[mutant-comment] crates/a/src/lib.rs:6: `// Mutants:` names `ghost`, but `t` never calls it",
        "[mutant-comment] crates/a/src/lib.rs:9: `// Mutants:` names `stray`, but is neither in a test nor directly above one",
    ];
    assert_eq!(messages(false), in_tests);
    // In a test file, even `shipped` is test code.
    assert_eq!(
        messages(true),
        [
            "[mutant-comment] crates/a/src/lib.rs:2: `// Mutants:` names `ghost`, but `shipped` never calls it",
            in_tests[0],
            in_tests[1],
        ]
    );
}

#[test]
fn reached_follows_test_helpers_but_not_shipped_code() {
    let file = source(
        "fn shipped() { from_shipped(); }\n#[cfg(test)]\nfn one() { two(); from_one(); }\n#[cfg(test)]\nfn two() { from_two(); t(); }\n#[test]\nfn t() { one(); shipped(); }\n",
        false,
    );
    let functions = &file.index.functions;
    let test = functions.last().unwrap();
    assert_eq!(
        reached(functions, test, false).bare,
        names(&["from_one", "from_two", "one", "shipped", "t", "two"])
    );
    // In a test file, `shipped` is a helper too.
    assert!(reached(functions, test, true).mentions("from_shipped"));
}

#[test]
fn a_block_gathers_claims_line_by_line() {
    let mut block = Block {
        start: 3,
        end: 3,
        claims: Vec::new(),
    };
    block.add(4, "// `one()` and `Two` and `a::three` and `x");
    assert_eq!(block.end, 4);
    assert_eq!(
        block.claims,
        [
            Claim {
                line: 4,
                path: String::from("one")
            },
            Claim {
                line: 4,
                path: String::from("a::three")
            },
        ]
    );
}

#[test]
fn plain_comments_and_identifiers_are_recognized() {
    assert!(is_plain_comment("// text"));
    assert!(is_plain_comment("//"));
    assert!(!is_plain_comment("/// doc"));
    assert!(!is_plain_comment("//! inner doc"));
    assert!(!is_plain_comment("code(); // trailing"));
    for good in ["a", "_a", "A9", "a_b", "snake_case"] {
        assert!(is_identifier(good), "{good}");
    }
    for bad in ["", "9a", "a-b", "a.b", " a"] {
        assert!(!is_identifier(bad), "{bad}");
    }
}
