//! A light index of one Rust source file for the pre-flight gate: its
//! functions, the lines each spans, whether each is test code, and
//! what each body calls by name.
//!
//! Name-based by design. There is no type resolution, so a call is
//! matched to a function by its name, and to a function in an `impl`
//! by `Type::name` as well. Calls inside macro bodies (`assert!`,
//! `assert_eq!`, ...) are found by scanning the macro's tokens, since
//! `syn` leaves them unparsed.

use std::collections::BTreeSet;

use proc_macro2::{Delimiter, Span, TokenStream, TokenTree};
use syn::visit::Visit;

/// What a body calls, by name.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct Calls {
    /// `name(…)`: a call by a bare name.
    pub(super) bare: BTreeSet<String>,
    /// `.name(…)`: a method call.
    pub(super) methods: BTreeSet<String>,
    /// `Qualifier::name`, called or passed by path.
    pub(super) paths: BTreeSet<String>,
    /// `name` as the last segment of a qualified path.
    pub(super) last: BTreeSet<String>,
}

impl Calls {
    /// Add every name `other` calls.
    pub(super) fn extend(&mut self, other: &Self) {
        self.bare.extend(other.bare.iter().cloned());
        self.methods.extend(other.methods.iter().cloned());
        self.paths.extend(other.paths.iter().cloned());
        self.last.extend(other.last.iter().cloned());
    }

    /// Whether `name` is called, or passed by path, in any form.
    pub(super) fn mentions(&self, name: &str) -> bool {
        self.bare.contains(name) || self.methods.contains(name) || self.last.contains(name)
    }

    /// A qualified path: its last segment, and `Qualifier::name`.
    fn qualified(&mut self, segments: &[String]) {
        if let [.., qualifier, name] = segments {
            self.last.insert(name.clone());
            self.paths.insert(format!("{qualifier}::{name}"));
        }
    }

    /// Scan a macro's tokens for `name(`, `.name(` and
    /// `Qualifier::name`.
    fn scan(&mut self, tokens: TokenStream) {
        let trees: Vec<TokenTree> = tokens.into_iter().collect();
        for (at, tree) in trees.iter().enumerate() {
            match tree {
                TokenTree::Group(group) => self.scan(group.stream()),
                TokenTree::Ident(ident) => self.token(&trees, at, ident.to_string()),
                TokenTree::Punct(_) | TokenTree::Literal(_) => {}
            }
        }
    }

    /// The identifier `name`, at `at` among `trees`.
    fn token(&mut self, trees: &[TokenTree], at: usize, name: String) {
        let back = |by: usize| at.checked_sub(by).and_then(|index| trees.get(index));
        let called = matches!(
            trees.get(at + 1),
            Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Parenthesis
        );
        if is_punct(back(1), ':') && is_punct(back(2), ':') {
            if let Some(TokenTree::Ident(qualifier)) = back(3) {
                self.paths.insert(format!("{qualifier}::{name}"));
            }
            self.last.insert(name);
        } else if called && is_punct(back(1), '.') {
            self.methods.insert(name);
        } else if called {
            self.bare.insert(name);
        }
    }
}

fn is_punct(tree: Option<&TokenTree>, ch: char) -> bool {
    matches!(tree, Some(TokenTree::Punct(punct)) if punct.as_char() == ch)
}

impl<'ast> Visit<'ast> for Calls {
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(callee) = &*call.func {
            if let Some(name) = callee.path.get_ident() {
                self.bare.insert(name.to_string());
            }
        }
        syn::visit::visit_expr_call(self, call);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        self.methods.insert(call.method.to_string());
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
        let segments: Vec<String> = path
            .path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect();
        self.qualified(&segments);
        syn::visit::visit_expr_path(self, path);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        self.scan(mac.tokens.clone());
        syn::visit::visit_macro(self, mac);
    }
}

/// A function with a body: a free function, a function in an `impl`,
/// or a trait's default method.
#[derive(Debug)]
pub(super) struct Function {
    pub(super) name: String,
    /// The `impl`'s type, or the trait, it belongs to.
    pub(super) owner: Option<String>,
    /// The trait its `impl` implements, if any.
    pub(super) implements: Option<String>,
    /// Whether it takes `self`.
    pub(super) receiver: bool,
    /// Whether its written return type is `!`: it never returns.
    pub(super) diverges: bool,
    /// Its first line, counting attributes and doc comments.
    pub(super) start: usize,
    /// The line of its `fn` keyword.
    pub(super) line: usize,
    /// The line of its closing brace.
    pub(super) end: usize,
    /// Test code: a `#[test]`, or anything under `#[cfg(test)]`.
    pub(super) test: bool,
    pub(super) calls: Calls,
}

impl Function {
    /// `Owner::name`, or `name`.
    pub(super) fn display(&self) -> String {
        self.owner.as_ref().map_or_else(
            || self.name.clone(),
            |owner| format!("{owner}::{}", self.name),
        )
    }

    /// Whether `calls` call this function directly: a free function by
    /// `name(…)` or a path ending in `::name`, a method by `.name(…)`
    /// or `Owner::name`, and an associated function only by
    /// `Owner::name`.
    pub(super) fn called_by(&self, calls: &Calls) -> bool {
        self.owner.as_ref().map_or_else(
            || calls.bare.contains(&self.name) || calls.last.contains(&self.name),
            |owner| {
                calls.paths.contains(&format!("{owner}::{}", self.name))
                    || (self.receiver && calls.methods.contains(&self.name))
            },
        )
    }
}

/// An out-of-line module declaration, `mod name;`.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct ModDecl {
    pub(super) name: String,
    /// Declared in test code (`#[cfg(test)] mod name;`).
    pub(super) test: bool,
}

/// Everything the gate needs from one file.
#[derive(Debug, Default)]
pub(super) struct Index {
    pub(super) functions: Vec<Function>,
    pub(super) mods: Vec<ModDecl>,
}

/// Index a source file.
pub(super) fn index(text: &str) -> syn::Result<Index> {
    let file = syn::parse_file(text)?;
    let mut index = Index::default();
    index.items(&file.items, false);
    Ok(index)
}

/// Where a function sits.
#[derive(Default)]
struct Owner {
    name: Option<String>,
    implements: Option<String>,
}

impl Index {
    fn items(&mut self, items: &[syn::Item], test: bool) {
        for item in items {
            if let syn::Item::Fn(function) = item {
                let owner = Owner::default();
                self.function(
                    &function.attrs,
                    &function.sig,
                    &function.block,
                    &owner,
                    test,
                );
            } else if let syn::Item::Impl(block) = item {
                self.impl_block(block, test || block.attrs.iter().any(is_cfg_test));
            } else if let syn::Item::Trait(definition) = item {
                self.trait_defaults(definition, test || definition.attrs.iter().any(is_cfg_test));
            } else if let syn::Item::Mod(module) = item {
                self.module(module, test || module.attrs.iter().any(is_cfg_test));
            }
        }
    }

    fn impl_block(&mut self, block: &syn::ItemImpl, test: bool) {
        let owner = Owner {
            name: Some(type_name(&block.self_ty)),
            implements: block
                .trait_
                .as_ref()
                .and_then(|(_, path, _)| path.segments.last())
                .map(|segment| segment.ident.to_string()),
        };
        for item in &block.items {
            if let syn::ImplItem::Fn(method) = item {
                self.function(&method.attrs, &method.sig, &method.block, &owner, test);
            }
        }
    }

    fn trait_defaults(&mut self, definition: &syn::ItemTrait, test: bool) {
        let owner = Owner {
            name: Some(definition.ident.to_string()),
            implements: None,
        };
        for item in &definition.items {
            if let syn::TraitItem::Fn(method) = item {
                if let Some(body) = &method.default {
                    self.function(&method.attrs, &method.sig, body, &owner, test);
                }
            }
        }
    }

    fn module(&mut self, module: &syn::ItemMod, test: bool) {
        if let Some((_, items)) = &module.content {
            self.items(items, test);
        } else {
            self.mods.push(ModDecl {
                name: module.ident.to_string(),
                test,
            });
        }
    }

    fn function(
        &mut self,
        attrs: &[syn::Attribute],
        sig: &syn::Signature,
        body: &syn::Block,
        owner: &Owner,
        test: bool,
    ) {
        let mut calls = Calls::default();
        calls.visit_block(body);
        let line = line_of(sig.fn_token.span);
        self.functions.push(Function {
            name: sig.ident.to_string(),
            owner: owner.name.clone(),
            implements: owner.implements.clone(),
            receiver: sig.receiver().is_some(),
            diverges: matches!(&sig.output, syn::ReturnType::Type(_, ty) if matches!(**ty, syn::Type::Never(_))),
            start: attrs
                .iter()
                .map(|attr| line_of(attr.pound_token.span))
                .fold(line, usize::min),
            line,
            end: line_of(body.brace_token.span.close()),
            test: test
                || attrs
                    .iter()
                    .any(|attr| is_test_attr(attr) || is_cfg_test(attr)),
            calls,
        });
    }
}

/// The last path segment of an `impl`'s type, through references;
/// `_` for any other type.
fn type_name(ty: &syn::Type) -> String {
    if let syn::Type::Path(path) = ty {
        path.path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
            .unwrap_or_default()
    } else if let syn::Type::Reference(reference) = ty {
        type_name(&reference.elem)
    } else {
        String::from("_")
    }
}

fn line_of(span: Span) -> usize {
    span.start().line
}

/// `#[test]`, or a crate's test attribute (`#[tokio::test]`).
fn is_test_attr(attr: &syn::Attribute) -> bool {
    attr.path()
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "test")
}

/// A `#[cfg(…)]` that names `test` and never `not`.
fn is_cfg_test(attr: &syn::Attribute) -> bool {
    let syn::Meta::List(list) = &attr.meta else {
        return false;
    };
    if !list.path.is_ident("cfg") {
        return false;
    }
    let text = list.tokens.to_string();
    let words: Vec<&str> = text
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .collect();
    words.contains(&"test") && !words.contains(&"not")
}

/// The lines multi-line literals in `text` continue onto, as first and
/// last inclusive: a `// Mutants:` inside a raw string is text, not a
/// comment. (A literal's own first line is code up to the literal.)
pub(super) fn multiline_literals(text: &str) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    if let Ok(tokens) = text.parse::<TokenStream>() {
        literals(tokens, &mut found);
    }
    found
}

fn literals(tokens: TokenStream, found: &mut Vec<(usize, usize)>) {
    for tree in tokens {
        match tree {
            TokenTree::Group(group) => literals(group.stream(), found),
            TokenTree::Literal(literal) => {
                let (first, last) = (literal.span().start().line, literal.span().end().line);
                if last > first {
                    found.push((first + 1, last));
                }
            }
            TokenTree::Ident(_) | TokenTree::Punct(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    fn only(text: &str) -> Function {
        let mut index = index(text).unwrap();
        assert_eq!(index.functions.len(), 1, "{:?}", index.functions);
        index.functions.remove(0)
    }

    #[test]
    fn a_function_diverges_only_when_its_written_return_type_is_never() {
        assert!(only("fn child() -> ! { loop {} }").diverges);
        assert!(only("impl T { fn stop(&self) -> ! { loop {} } }").diverges);
        for text in [
            "fn unit() {}",
            "fn value() -> u8 { 0 }",
            "fn wrapped() -> Result<(), E> { Ok(()) }",
            "fn aliased() -> Never { loop {} }",
        ] {
            assert!(!only(text).diverges, "{text}");
        }
    }

    #[test]
    fn a_body_records_each_form_of_call() {
        let function = only(
            "fn t() {
                bare(1);
                value.method();
                Owner::assoc();
                module::free();
                list.iter().map(Kind::convert);
                let local = variable;
                assert_eq!(in_macro(x.in_method()), Type::in_path());
                assert!(nested(vec![deep()]));
            }",
        );
        let calls = function.calls;
        assert_eq!(calls.bare, set(&["bare", "deep", "in_macro", "nested"]));
        assert_eq!(calls.methods, set(&["in_method", "iter", "map", "method"]));
        assert_eq!(
            calls.paths,
            set(&[
                "Kind::convert",
                "Owner::assoc",
                "Type::in_path",
                "module::free"
            ])
        );
        assert_eq!(calls.last, set(&["assoc", "convert", "free", "in_path"]));
    }

    #[test]
    fn a_macro_path_needs_both_colons_and_a_qualifier_for_its_path() {
        let calls = only("fn t() { m!(a : b(), ::lead(), x . y, z.w(), q :: r); }").calls;
        // `a : b()` is a bare call; `::lead` has no qualifier but is a
        // path; `x . y` is not called; `q :: r` is a path.
        assert_eq!(calls.bare, set(&["b"]));
        assert_eq!(calls.methods, set(&["w"]));
        assert_eq!(calls.paths, set(&["q::r"]));
        assert_eq!(calls.last, set(&["lead", "r"]));
    }

    #[test]
    fn a_macro_call_needs_parentheses() {
        let calls = only("fn t() { m!(a[0], b{}, c(), d.e[1], f.g()); }").calls;
        assert_eq!(calls.bare, set(&["c"]));
        assert_eq!(calls.methods, set(&["g"]));
    }

    #[test]
    fn mentions_and_extend_cover_every_set() {
        let mut calls = Calls::default();
        assert!(!calls.mentions("x"));
        let other = Calls {
            bare: set(&["a"]),
            methods: set(&["b"]),
            paths: set(&["T::c"]),
            last: set(&["c"]),
        };
        calls.extend(&other);
        assert_eq!(calls, other);
        for name in ["a", "b", "c"] {
            assert!(calls.mentions(name), "{name}");
        }
        assert!(!calls.mentions("T::c"));
    }

    #[test]
    fn functions_carry_their_lines_owner_and_receiver() {
        let index = index(
            "/// Doc.
#[inline]
pub fn free() {
}
impl Thing {
    fn method(&self) {}
    fn assoc() -> Self { Self }
}
impl Display for &Thing {
    fn fmt(&self) {}
}
impl<T> Wrapper<T> for [T] {
    fn odd(self) {}
}
trait Kind {
    fn required(&self);
    fn provided(&self) { self.required() }
}
",
        )
        .unwrap();
        let summary: Vec<_> = index
            .functions
            .iter()
            .map(|f| {
                (
                    f.display(),
                    f.implements.clone(),
                    f.receiver,
                    f.start,
                    f.line,
                    f.end,
                    f.test,
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                (String::from("free"), None, false, 1, 3, 4, false),
                (String::from("Thing::method"), None, true, 6, 6, 6, false),
                (String::from("Thing::assoc"), None, false, 7, 7, 7, false),
                (
                    String::from("Thing::fmt"),
                    Some(String::from("Display")),
                    true,
                    10,
                    10,
                    10,
                    false
                ),
                (
                    String::from("_::odd"),
                    Some(String::from("Wrapper")),
                    true,
                    13,
                    13,
                    13,
                    false
                ),
                (
                    String::from("Kind::provided"),
                    None,
                    true,
                    17,
                    17,
                    17,
                    false
                ),
            ]
        );
    }

    #[test]
    fn test_code_is_marked_wherever_it_sits() {
        let index = index(
            "fn prod() {}
#[test]
fn plain() {}
#[tokio::test]
async fn async_test() {}
#[cfg(test)]
fn helper() {}
#[cfg(test)]
mod tests {
    fn inner() {}
    mod deeper { fn deepest() {} }
}
#[cfg(not(test))]
mod real { fn shipped() {} }
#[cfg(all(test, unix))]
impl Thing { fn test_impl() {} }
#[cfg(test)]
trait Fake { fn fake(&self) {} }
#[cfg(unix)]
mod unix { fn platform() {} }
#[test_case]
fn not_a_test_attr() {}
#[cfg(test)]
mod outside;
mod shipped_file;
",
        )
        .unwrap();
        let marked: Vec<_> = index
            .functions
            .iter()
            .map(|f| (f.name.as_str(), f.test))
            .collect();
        assert_eq!(
            marked,
            [
                ("prod", false),
                ("plain", true),
                ("async_test", true),
                ("helper", true),
                ("inner", true),
                ("deepest", true),
                ("shipped", false),
                ("test_impl", true),
                ("fake", true),
                ("platform", false),
                ("not_a_test_attr", false),
            ]
        );
        assert_eq!(
            index.mods,
            [
                ModDecl {
                    name: String::from("outside"),
                    test: true
                },
                ModDecl {
                    name: String::from("shipped_file"),
                    test: false
                },
            ]
        );
    }

    #[test]
    fn a_test_module_marks_out_of_line_children() {
        let index = index("#[cfg(test)]\nmod tests { mod helper; }").unwrap();
        assert_eq!(
            index.mods,
            [ModDecl {
                name: String::from("helper"),
                test: true
            }]
        );
    }

    #[test]
    fn a_cfg_list_that_is_not_cfg_is_not_test() {
        let index = index("#[allow(test)]\nfn a() {}\n#[doc = \"test\"]\nfn b() {}").unwrap();
        assert!(index.functions.iter().all(|f| !f.test));
    }

    #[test]
    fn called_by_matches_each_kind_of_function_its_own_way() {
        let calls = Calls {
            bare: set(&["free"]),
            methods: set(&["method", "assoc_by_method"]),
            paths: set(&["Thing::assoc", "module::by_path"]),
            last: set(&["assoc", "by_path"]),
        };
        let function = |name: &str, owner: Option<&str>, receiver: bool| Function {
            name: name.to_owned(),
            owner: owner.map(str::to_owned),
            implements: None,
            receiver,
            diverges: false,
            start: 1,
            line: 1,
            end: 1,
            test: false,
            calls: Calls::default(),
        };
        assert!(function("free", None, false).called_by(&calls));
        assert!(function("by_path", None, false).called_by(&calls));
        assert!(!function("method", None, false).called_by(&calls));
        assert!(function("method", Some("Thing"), true).called_by(&calls));
        assert!(!function("method", Some("Thing"), false).called_by(&calls));
        assert!(function("assoc", Some("Thing"), false).called_by(&calls));
        assert!(!function("assoc", Some("Other"), false).called_by(&calls));
        assert!(!function("assoc_by_method", Some("Thing"), false).called_by(&calls));
        assert!(!function("free", Some("Thing"), true).called_by(&calls));
    }

    #[test]
    fn multiline_literals_are_found_and_single_lines_are_not() {
        let text = "fn t() {\n    let a = \"one line\";\n    let b = r#\"first\n// Mutants: `x`\nlast\"#;\n    m!(\"in\na group\");\n}\n";
        assert_eq!(multiline_literals(text), [(4, 5), (7, 7)]);
        assert!(multiline_literals("fn t() { \"unterminated }").is_empty());
    }
    fn attrs(text: &str) -> Vec<syn::Attribute> {
        use syn::parse::Parser as _;
        syn::Attribute::parse_outer.parse_str(text).unwrap()
    }

    #[test]
    fn a_qualified_path_records_its_last_two_segments() {
        let mut calls = Calls::default();
        calls.qualified(&[String::from("solo")]);
        calls.qualified(&[]);
        assert_eq!(calls, Calls::default());
        calls.qualified(&["a", "b", "c"].map(String::from));
        assert_eq!(calls.paths, set(&["b::c"]));
        assert_eq!(calls.last, set(&["c"]));
    }

    #[test]
    fn scanning_tokens_finds_calls_at_every_depth() {
        let mut calls = Calls::default();
        calls.scan("f(x) [g(y.h())] {T::i}".parse().unwrap());
        assert_eq!(calls.bare, set(&["f", "g"]));
        assert_eq!(calls.methods, set(&["h"]));
        assert_eq!(calls.paths, set(&["T::i"]));
    }

    #[test]
    fn one_token_is_read_by_its_neighbours() {
        let trees: Vec<TokenTree> = ". a () : : b c ()"
            .parse::<TokenStream>()
            .unwrap()
            .into_iter()
            .collect();
        let read = |at: usize, name: &str| {
            let mut calls = Calls::default();
            calls.token(&trees, at, name.to_owned());
            calls
        };
        assert_eq!(read(1, "a").methods, set(&["a"]));
        let path = read(5, "b");
        assert_eq!(path.last, set(&["b"]));
        assert!(path.paths.is_empty(), "`()` is no qualifier");
        assert_eq!(read(6, "c").bare, set(&["c"]));
        assert_eq!(read(7, "unused"), Calls::default());
    }

    #[test]
    fn is_punct_matches_one_character_only() {
        let trees: Vec<TokenTree> = ": x".parse::<TokenStream>().unwrap().into_iter().collect();
        assert!(is_punct(trees.first(), ':'));
        assert!(!is_punct(trees.first(), '.'));
        assert!(!is_punct(trees.get(1), ':'));
        assert!(!is_punct(None, ':'));
    }

    fn call(text: &str) -> Option<syn::ExprCall> {
        if let syn::Expr::Call(call) = syn::parse_str(text).ok()? {
            Some(call)
        } else {
            None
        }
    }

    fn method_call(text: &str) -> Option<syn::ExprMethodCall> {
        if let syn::Expr::MethodCall(call) = syn::parse_str(text).ok()? {
            Some(call)
        } else {
            None
        }
    }

    #[test]
    fn each_visitor_records_its_own_form() {
        let mut calls = Calls::default();
        calls.visit_expr_call(&call("free(inner())").unwrap());
        calls.visit_expr_call(&call("T::assoc()").unwrap());
        assert_eq!(calls.bare, set(&["free", "inner"]));
        assert_eq!(calls.paths, set(&["T::assoc"]));
        let mut calls = Calls::default();
        calls.visit_expr_method_call(&method_call("x.outer(y.inner())").unwrap());
        assert_eq!(calls.methods, set(&["inner", "outer"]));
        let mut calls = Calls::default();
        calls.visit_expr_path(&syn::parse_str("a::b").unwrap());
        assert_eq!(calls.paths, set(&["a::b"]));
        let mut calls = Calls::default();
        let mac: syn::ExprMacro = syn::parse_str("m!(f(), n!(g()))").unwrap();
        calls.visit_macro(&mac.mac);
        assert_eq!(calls.bare, set(&["f", "g"]));
    }

    #[test]
    fn index_parts_record_what_they_are_given() {
        let file = syn::parse_file("fn a() {}\nmod m { fn b() {} }").unwrap();
        let mut index = Index::default();
        index.items(&file.items, true);
        let names: Vec<_> = index
            .functions
            .iter()
            .map(|f| (f.name.as_str(), f.test))
            .collect();
        assert_eq!(names, [("a", true), ("b", true)]);

        let mut index = Index::default();
        let block: syn::ItemImpl =
            syn::parse_str("impl Iterator for It { fn next(&mut self) {} }").unwrap();
        index.impl_block(&block, false);
        let definition: syn::ItemTrait =
            syn::parse_str("trait T { fn d(&self) { x() } fn r(&self); }").unwrap();
        index.trait_defaults(&definition, true);
        let shown: Vec<_> = index
            .functions
            .iter()
            .map(|f| (f.display(), f.implements.clone(), f.test))
            .collect();
        assert_eq!(
            shown,
            [
                (
                    String::from("It::next"),
                    Some(String::from("Iterator")),
                    false
                ),
                (String::from("T::d"), None, true),
            ]
        );

        let mut index = Index::default();
        index.module(&syn::parse_str("mod outer;").unwrap(), true);
        index.module(&syn::parse_str("mod inner { fn c() {} }").unwrap(), false);
        assert_eq!(
            index.mods,
            [ModDecl {
                name: String::from("outer"),
                test: true
            }]
        );
        assert_eq!(index.functions.len(), 1);

        let mut index = Index::default();
        let function: syn::ItemFn = syn::parse_str("\n#[test]\nfn t() {\n    go();\n}").unwrap();
        let owner = Owner {
            name: Some(String::from("O")),
            implements: Some(String::from("Tr")),
        };
        index.function(
            &function.attrs,
            &function.sig,
            &function.block,
            &owner,
            false,
        );
        let f = index.functions.first().unwrap();
        assert_eq!(
            (
                f.display(),
                f.implements.as_deref(),
                f.start,
                f.line,
                f.end,
                f.test
            ),
            (String::from("O::t"), Some("Tr"), 2, 3, 5, true)
        );
        assert_eq!(f.calls.bare, set(&["go"]));
    }

    #[test]
    fn a_type_is_named_by_its_last_segment() {
        for (ty, name) in [
            ("a::B<T>", "B"),
            ("&mut &C", "C"),
            ("[T]", "_"),
            ("(A, B)", "_"),
        ] {
            assert_eq!(type_name(&syn::parse_str(ty).unwrap()), name, "{ty}");
        }
    }

    #[test]
    fn a_span_gives_its_line() {
        let function: syn::ItemFn = syn::parse_str("\n\n\nfn f() {}").unwrap();
        assert_eq!(line_of(function.sig.fn_token.span), 4);
        assert_eq!(line_of(function.sig.ident.span()), 4);
    }

    #[test]
    fn test_attributes_and_cfgs_are_told_apart() {
        let cases = [
            ("#[test]", true, false),
            ("#[tokio::test]", true, false),
            ("#[test_case]", false, false),
            ("#[cfg(test)]", false, true),
            ("#[cfg(all(test, unix))]", false, true),
            ("#[cfg(not(test))]", false, false),
            ("#[cfg(unix)]", false, false),
            ("#[cfg(feature = \"test_mode\")]", false, false),
            ("#[allow(test)]", false, false),
            ("#[cfg]", false, false),
        ];
        for (text, test, cfg_test) in cases {
            let attr = attrs(text).remove(0);
            assert_eq!(is_test_attr(&attr), test, "{text}");
            assert_eq!(is_cfg_test(&attr), cfg_test, "{text}");
        }
    }

    #[test]
    fn literals_records_continuation_lines_at_any_depth() {
        let mut found = Vec::new();
        literals(
            "a \"x\" (\"b\nc\nd\") [r\"e\nf\"]".parse().unwrap(),
            &mut found,
        );
        assert_eq!(found, [(2, 3), (4, 4)]);
    }
}
