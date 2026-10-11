//! Mutation pre-flight gate: two cheap checks that refuse a
//! mutation run before it launches.
//!
//! `hatchling-dev preflight --base <commit> [--root <dir>]` checks the
//! Rust files changed since `<commit>`, the WO base: committed and
//! uncommitted changes and new files, everything a run would measure.
//! Each finding names its `file:line`, and any finding fails the gate:
//! `scripts/mutant-run.sh` then refuses to launch.
//!
//! # The mutant-comment lint
//!
//! The comment form is pinned. A claim is made only by a line comment
//! that starts `// Mutants:` (after indentation), continued by the plain
//! `//` lines directly below it. Each backtick-quoted function path in
//! it is a claim: `` `name` ``, `` `name()` `` or `` `Type::name` ``,
//! whose last segment is a lowercase name, not a keyword or a primitive
//! type. Everything else is prose, including unquoted names and quoted
//! operators such as `` `>=` ``.
//!
//! A comment inside a test function belongs to it, and so does one on
//! the line directly above a test's attributes. A claim holds when that
//! test calls the named function, matched by its last path segment,
//! itself or through a test helper in the same file. A claim that
//! belongs to no function fails as well. A comment in non-test code is
//! prose.
//!
//! # The delegation check
//!
//! Every changed non-test function must be called directly by test
//! code: a `#[test]` function, anything under `#[cfg(test)]` (including
//! out-of-line test modules), or any file in a crate's `tests/`
//! directory, in any crate of the workspace. Being covered through
//! another function does not count: a one-line delegation tested only
//! through its delegate is exactly the mutant that survives.
//!
//! A free function counts as called by `name(…)` or by a path ending
//! in `::name`; a method by `.name(…)` or `Type::name`; an associated
//! function (no `self`) only by `Type::name`. These are names, not
//! types: there is no type resolution, so a same-named function
//! elsewhere can stand in for one. Three kinds of function are skipped,
//! because no test calls them by name: a binary's `main` (in
//! `src/main.rs`, `src/bin/`, or a file a `[[bin]]` target names with
//! `path` in its crate's `Cargo.toml`), the
//! methods of trait impls that the language or `std` calls for you
//! (`Display::fmt`, `Drop::drop`, `Iterator::next`, `From::from`, ...;
//! see `SYNTAX_TRAITS`), and diverging functions, whose written return
//! type is `!`. A function that never returns runs only as a
//! process's last act (a forked child's entry point, an `exec`), so a
//! test can call it only by forking; and cargo-mutants has no value to
//! replace its body with. What it delegates to is still checked: each
//! function it calls is held to this rule on its own. The type must be
//! written `!`: an alias for it is not recognised.
//!
//! # Scope
//!
//! `crates/*/src` and `crates/*/tests`. A file that does not parse is
//! not compiled code, and it is skipped, unless it changed: then the
//! gate cannot check it, and fails.

mod index;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use self::index::{Calls, Function, Index};
use super::Finding;
use crate::Error;

/// The finding tag of the mutant-comment lint.
pub const COMMENT_CHECK: &str = "mutant-comment";

/// The finding tag of the delegation check.
pub const DELEGATION_CHECK: &str = "delegation";

/// The pinned comment form.
const MARKER: &str = "// Mutants:";

/// Bound for each `git` call: a diff of a task's changes.
const GIT_TIMEOUT: Duration = Duration::from_secs(120);

/// Traits whose methods the language or `std` calls, rather than a
/// test by name: operators, formatting, conversions, iteration, drop.
const SYNTAX_TRAITS: &[&str] = &[
    "Add",
    "AddAssign",
    "AsMut",
    "AsRef",
    "Borrow",
    "BorrowMut",
    "Clone",
    "Debug",
    "Default",
    "Deref",
    "DerefMut",
    "Deserialize",
    "Display",
    "Div",
    "Drop",
    "Eq",
    "Error",
    "Extend",
    "From",
    "FromIterator",
    "FromStr",
    "Future",
    "Hash",
    "Index",
    "IndexMut",
    "IntoIterator",
    "Iterator",
    "Mul",
    "Neg",
    "Not",
    "Ord",
    "PartialEq",
    "PartialOrd",
    "Read",
    "Rem",
    "Serialize",
    "Sub",
    "TryFrom",
    "Write",
];

/// Lowercase words that look like function names in a claim but are
/// keywords or primitive types.
const NOT_FUNCTIONS: &[&str] = &[
    "as", "async", "await", "bool", "break", "char", "const", "continue", "crate", "dyn", "else",
    "enum", "f32", "f64", "false", "fn", "for", "i128", "i16", "i32", "i64", "i8", "if", "impl",
    "in", "isize", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self",
    "static", "str", "struct", "super", "trait", "true", "type", "u128", "u16", "u32", "u64", "u8",
    "unsafe", "use", "usize", "where", "while",
];

/// Run both checks on the Rust files changed since `base`.
///
/// # Errors
///
/// Returns [`Error::ToolFailed`] for a `base` that is empty or looks
/// like an option, or when `git` cannot diff against it;
/// [`Error::Parse`] when a changed file does not parse; and
/// [`Error::Io`] when a source file cannot be read.
pub fn run(root: &Path, base: &str) -> Result<Vec<Finding>, Error> {
    if base.is_empty() || base.starts_with('-') {
        return Err(Error::ToolFailed {
            tool: String::from("preflight"),
            status: format!("bad --base `{base}`: expected a commit"),
        });
    }
    let changes = changes(root, base)?;
    let tree = Tree::load(root, &changes)?;
    let test_calls = tree.test_calls();
    let mut findings = Vec::new();
    for (path, file_changes) in &changes {
        if let Some(file) = tree.files.get(path) {
            findings.extend(comment_findings(path, file));
            if !file.test {
                findings.extend(delegation_findings(
                    path,
                    file,
                    file_changes,
                    &test_calls,
                    &tree.entry_points,
                ));
            }
        }
    }
    Ok(findings)
}

/// What changed in one file, in new-file line numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Change {
    /// Lines `first..=last` were added or replaced.
    Lines { first: usize, last: usize },
    /// Lines were deleted after line `after`.
    Deleted { after: usize },
}

/// Every Rust file changed since `base`, with what changed: the diff
/// to the working tree, plus untracked files whole.
fn changes(root: &Path, base: &str) -> Result<BTreeMap<String, Vec<Change>>, Error> {
    let diff = git(
        root,
        &[
            "-c",
            "core.quotepath=off",
            "diff",
            "--no-color",
            "--no-ext-diff",
            "-U0",
            base,
            "--",
        ],
    )?;
    let mut changes = parse_diff(&diff);
    for path in git(root, &["ls-files", "--others", "--exclude-standard"])?.lines() {
        changes
            .entry(path.to_owned())
            .or_default()
            .push(Change::Lines {
                first: 1,
                last: usize::MAX,
            });
    }
    Ok(changes)
}

fn git(root: &Path, args: &[&str]) -> Result<String, Error> {
    let driven = super::driver::drive(&super::driver::Spec {
        program: "git",
        args,
        current_dir: Some(root),
        env: &[],
        timeout: GIT_TIMEOUT,
    })?;
    if !driven.status.success() {
        return Err(Error::ToolFailed {
            tool: format!("git {}", args.join(" ")),
            status: String::from_utf8_lossy(&driven.stderr).trim().to_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&driven.stdout).into_owned())
}

/// The changed lines of each file in a `git diff -U0`.
fn parse_diff(diff: &str) -> BTreeMap<String, Vec<Change>> {
    let mut changes: BTreeMap<String, Vec<Change>> = BTreeMap::new();
    let mut file: Option<&str> = None;
    for line in diff.lines() {
        if let Some(target) = line.strip_prefix("+++ ") {
            // `/dev/null` (a deleted file) has no `b/` side.
            file = target.strip_prefix("b/");
        } else if let Some(hunk) = line.strip_prefix("@@ ") {
            if let (Some(path), Some(change)) = (file, hunk_change(hunk)) {
                changes.entry(path.to_owned()).or_default().push(change);
            }
        }
    }
    changes
}

/// The new side of a hunk header, `-a,b +c,d @@`.
fn hunk_change(hunk: &str) -> Option<Change> {
    let new = hunk.split_whitespace().nth(1)?.strip_prefix('+')?;
    let (first, count) = new.split_once(',').unwrap_or((new, "1"));
    let first: usize = first.parse().ok()?;
    let count: usize = count.parse().ok()?;
    Some(if count == 0 {
        Change::Deleted { after: first }
    } else {
        Change::Lines {
            first,
            last: first + count - 1,
        }
    })
}

/// Whether a change falls in a function, from its `fn` line to its
/// closing brace: attributes and doc comments make no mutants.
fn touched(function: &Function, changes: &[Change]) -> bool {
    changes.iter().any(|change| match *change {
        Change::Lines { first, last } => first <= function.end && function.line <= last,
        Change::Deleted { after } => function.line <= after && after < function.end,
    })
}

/// One indexed source file.
struct Source {
    text: String,
    index: Index,
    /// The whole file is test code.
    test: bool,
}

/// Every indexed source file of the workspace, by relative path.
struct Tree {
    files: BTreeMap<String, Source>,
    /// The files `[[bin]]` targets name with `path` (see `entry_points`).
    entry_points: BTreeSet<String>,
}

impl Tree {
    fn load(root: &Path, changes: &BTreeMap<String, Vec<Change>>) -> Result<Self, Error> {
        let mut files = BTreeMap::new();
        for path in rust_files(root)? {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            let text = std::fs::read_to_string(&path).map_err(Error::Io)?;
            match index::index(&text) {
                Ok(index) => {
                    let in_tests = relative.split('/').nth(2) == Some("tests");
                    let source = Source {
                        text,
                        index,
                        test: in_tests,
                    };
                    files.insert(relative, source);
                }
                Err(error) if changes.contains_key(&relative) => {
                    return Err(Error::Parse {
                        path,
                        reason: error.to_string(),
                    });
                }
                Err(_) => {}
            }
        }
        mark_test_modules(&mut files);
        Ok(Self {
            files,
            entry_points: entry_points(root)?,
        })
    }

    /// Everything test code calls, across the workspace.
    fn test_calls(&self) -> Calls {
        let mut calls = Calls::default();
        for file in self.files.values() {
            for function in &file.index.functions {
                if file.test || function.test {
                    calls.extend(&function.calls);
                }
            }
        }
        calls
    }
}

/// Every `.rs` file under `crates/*/src` and `crates/*/tests`.
fn rust_files(root: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut found = Vec::new();
    let crates = std::fs::read_dir(root.join("crates")).map_err(Error::Io)?;
    for entry in crates {
        let dir = entry.map_err(Error::Io)?.path();
        for part in ["src", "tests"] {
            collect(&dir.join(part), &mut found)?;
        }
    }
    Ok(found)
}

/// The files that `[[bin]]` targets name with `path` in each crate's
/// `Cargo.toml`: binary entry points outside `src/main.rs` and
/// `src/bin/`, by relative path. A crate without a manifest has none.
fn entry_points(root: &Path) -> Result<BTreeSet<String>, Error> {
    let mut found = BTreeSet::new();
    let crates = std::fs::read_dir(root.join("crates")).map_err(Error::Io)?;
    for entry in crates {
        let dir = entry.map_err(Error::Io)?.path();
        let manifest = dir.join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        let table = super::read_toml(&manifest)?;
        let bins = table
            .get("bin")
            .and_then(toml::Value::as_array)
            .map_or(&[][..], Vec::as_slice);
        for path in bins
            .iter()
            .filter_map(|bin| bin.get("path"))
            .filter_map(toml::Value::as_str)
        {
            let file = dir.join(path.trim_start_matches("./"));
            let relative = file.strip_prefix(root).unwrap_or(&file);
            found.insert(relative.to_string_lossy().into_owned());
        }
    }
    Ok(found)
}

fn collect(dir: &Path, found: &mut Vec<PathBuf>) -> Result<(), Error> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries {
        let path = entry.map_err(Error::Io)?.path();
        if path.is_dir() {
            collect(&path, found)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
    Ok(())
}

/// Out-of-line modules declared in test code, or in a test file, are
/// test files too (`#[cfg(test)] mod test_socket;`).
fn mark_test_modules(files: &mut BTreeMap<String, Source>) {
    // Each round marks at least one file, so this many rounds always
    // reach the end.
    for _ in 0..=files.len() {
        let found: Vec<String> = files
            .iter()
            .flat_map(|(path, file)| {
                file.index
                    .mods
                    .iter()
                    .filter(|decl| decl.test || file.test)
                    .filter_map(|decl| module_file(files, path, &decl.name))
            })
            .filter(|child| files.get(child).is_some_and(|file| !file.test))
            .collect();
        if found.is_empty() {
            return;
        }
        for child in found {
            if let Some(file) = files.get_mut(&child) {
                file.test = true;
            }
        }
    }
}

/// The file `mod name;` in `parent` loads: `name.rs` or `name/mod.rs`,
/// beside a `lib.rs`, `main.rs`, `mod.rs` or crate root in `tests/` or
/// `src/bin/`, and otherwise in the directory named for `parent`.
fn module_file(files: &BTreeMap<String, Source>, parent: &str, name: &str) -> Option<String> {
    let (dir, file) = parent.rsplit_once('/')?;
    let beside = matches!(file, "lib.rs" | "main.rs" | "mod.rs")
        || dir.ends_with("/tests")
        || dir.ends_with("/src/bin");
    let base = if beside {
        dir.to_owned()
    } else {
        format!("{dir}/{}", file.strip_suffix(".rs")?)
    };
    [format!("{base}/{name}.rs"), format!("{base}/{name}/mod.rs")]
        .into_iter()
        .find(|candidate| files.contains_key(candidate))
}

fn delegation_findings(
    path: &str,
    file: &Source,
    changes: &[Change],
    test_calls: &Calls,
    entry_points: &BTreeSet<String>,
) -> Vec<Finding> {
    file.index
        .functions
        .iter()
        .filter(|function| {
            !function.test
                && touched(function, changes)
                && !exempt(path, function, entry_points)
                && !function.called_by(test_calls)
        })
        .map(|function| {
            Finding::new(
                DELEGATION_CHECK,
                format!(
                    "{path}:{}: `{}` changed, but no test calls it directly",
                    function.line,
                    function.display()
                ),
            )
        })
        .collect()
}

/// A binary's `main`, methods of the traits in `SYNTAX_TRAITS`, and
/// diverging functions.
fn exempt(path: &str, function: &Function, entry_points: &BTreeSet<String>) -> bool {
    let entry_point = function.owner.is_none()
        && function.name == "main"
        && (path.ends_with("/main.rs")
            || path.contains("/src/bin/")
            || entry_points.contains(path));
    entry_point
        || function.diverges
        || function
            .implements
            .as_deref()
            .is_some_and(|name| SYNTAX_TRAITS.contains(&name))
}

/// A function path quoted in a `// Mutants:` comment.
#[derive(Debug, PartialEq, Eq)]
struct Claim {
    line: usize,
    path: String,
}

impl Claim {
    /// The last segment: what a test must call.
    fn name(&self) -> &str {
        self.path.rsplit("::").next().unwrap_or(&self.path)
    }
}

/// A `// Mutants:` comment: its lines and its claims.
#[derive(Debug, PartialEq, Eq)]
struct Block {
    start: usize,
    end: usize,
    claims: Vec<Claim>,
}

impl Block {
    fn add(&mut self, line: usize, text: &str) {
        self.end = line;
        self.claims.extend(
            quoted(text)
                .filter(|quote| is_function_path(quote))
                .map(|quote| Claim {
                    line,
                    path: quote.strip_suffix("()").unwrap_or(quote).to_owned(),
                }),
        );
    }
}

/// Every `// Mutants:` comment in `text`, outside string literals.
fn blocks(text: &str) -> Vec<Block> {
    let literals = index::multiline_literals(text);
    let mut blocks = Vec::new();
    let mut open: Option<Block> = None;
    for (number, raw) in (1..).zip(text.lines()) {
        let line = raw.trim_start();
        let code = !literals
            .iter()
            .any(|&(first, last)| first <= number && number <= last);
        if code && line.starts_with(MARKER) {
            blocks.extend(open.take());
            let mut block = Block {
                start: number,
                end: number,
                claims: Vec::new(),
            };
            block.add(number, line.strip_prefix(MARKER).unwrap_or(line));
            open = Some(block);
        } else if code && is_plain_comment(line) {
            if let Some(block) = open.as_mut() {
                block.add(number, line);
            }
        } else {
            blocks.extend(open.take());
        }
    }
    blocks.extend(open);
    blocks
}

fn is_plain_comment(line: &str) -> bool {
    line.starts_with("//") && !line.starts_with("///") && !line.starts_with("//!")
}

/// The backtick-quoted spans of `text`; an unmatched backtick quotes
/// nothing.
fn quoted(text: &str) -> impl Iterator<Item = &str> {
    let parts: Vec<&str> = text.split('`').collect();
    let pairs = parts.len().saturating_sub(1) / 2;
    parts.into_iter().skip(1).step_by(2).take(pairs)
}

/// `name`, `name()` or `Path::name`, ending in a lowercase name that
/// is not a keyword or a primitive type.
fn is_function_path(quote: &str) -> bool {
    let path = quote.strip_suffix("()").unwrap_or(quote);
    let name = path.rsplit("::").next().unwrap_or(path);
    path.split("::").all(is_identifier)
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && name.chars().any(|c| c.is_ascii_lowercase())
        && !NOT_FUNCTIONS.contains(&name)
}

fn is_identifier(segment: &str) -> bool {
    let mut chars = segment.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The function a comment belongs to: the one it sits in, or else
/// the one starting on the line below it. Indexed functions never
/// nest (items inside a body are not indexed), so at most one holds it.
fn attach<'a>(functions: &'a [Function], block: &Block) -> Option<&'a Function> {
    functions
        .iter()
        .find(|function| function.start <= block.start && block.end <= function.end)
        .or_else(|| {
            functions
                .iter()
                .find(|function| function.start == block.end + 1)
        })
}

/// What `test` calls, itself or through the test helpers it calls in
/// the same file.
fn reached(functions: &[Function], test: &Function, test_file: bool) -> Calls {
    let mut reached = test.calls.clone();
    let mut merged: BTreeSet<usize> = BTreeSet::new();
    // Each round merges at least one helper, so this many rounds
    // always reach the end.
    for _ in 0..=functions.len() {
        let next: Vec<(usize, &Function)> = functions
            .iter()
            .enumerate()
            .filter(|&(at, helper)| {
                (helper.test || test_file)
                    && !std::ptr::eq(helper, test)
                    && !merged.contains(&at)
                    && helper.called_by(&reached)
            })
            .collect();
        if next.is_empty() {
            break;
        }
        for (at, helper) in next {
            reached.extend(&helper.calls);
            merged.insert(at);
        }
    }
    reached
}

fn comment_findings(path: &str, file: &Source) -> Vec<Finding> {
    let functions = &file.index.functions;
    let mut findings = Vec::new();
    for block in blocks(&file.text) {
        let Some(owner) = attach(functions, &block) else {
            findings.extend(block.claims.iter().map(|claim| {
                Finding::new(
                    COMMENT_CHECK,
                    format!(
                        "{path}:{}: `// Mutants:` names `{}`, but is neither in a test nor directly above one",
                        claim.line, claim.path
                    ),
                )
            }));
            continue;
        };
        if !(owner.test || file.test) {
            continue;
        }
        let reached = reached(functions, owner, file.test);
        findings.extend(
            block
                .claims
                .iter()
                .filter(|claim| !reached.mentions(claim.name()))
                .map(|claim| {
                    Finding::new(
                        COMMENT_CHECK,
                        format!(
                            "{path}:{}: `// Mutants:` names `{}`, but `{}` never calls it",
                            claim.line,
                            claim.path,
                            owner.display()
                        ),
                    )
                }),
        );
    }
    findings
}

#[cfg(test)]
mod tests;
