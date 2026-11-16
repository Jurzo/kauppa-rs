//! MB-0057 (RR-0049 constraint 3): every route that changes what a list
//! shows announces it. A source scan, like `csrf::tests::coverage`: a new
//! `ShoppingListRepo` method must be classified here, and a function that
//! calls a write must announce in the same function.
#![allow(clippy::expect_used, clippy::panic)]

use crate::routes::test_support::source_files;

/// `ShoppingListRepo`'s methods that only read.
const READS: &[&str] = &[
    "list_for_owner",
    "shared_with",
    "archived",
    "get",
    "access",
    "items",
    "store_view",
    "shares",
];

/// Its methods that change what a list shows: a caller announces.
const WRITES: &[&str] = &[
    "archive",
    "unarchive",
    "rename",
    "delete",
    "add_item",
    "add_product",
    "add_owner_favorite_by_name",
    "add_recipe",
    "assign_category",
    "set_item_quantity",
    "update_item",
    "set_checked",
    "remove_item",
    "restore_item",
    "share",
    "revoke",
];

/// Writes that aren't announced: nobody is viewing a list that was just
/// created (RR-0049 sub-answer 6).
const UNANNOUNCED: &[&str] = &["create"];

/// The `CategoryRepo` writes that change what the owner's lists show
/// (names and order): their callers announce with `owner_changed`.
const CATEGORY_WRITES: &[&str] = &[".pool.rename(", ".pool.delete(", ".pool.reorder("];

/// What counts as announcing.
const ANNOUNCES: &[&str] = &["announce(", ".live.changed("];

/// The method names of `trait ShoppingListRepo`, from kauppa-core's source.
fn repo_methods() -> Vec<String> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../kauppa-core/src/repo/mod.rs"
    );
    let source = std::fs::read_to_string(path).expect("kauppa-core's repo/mod.rs");
    let start = source
        .find("pub trait ShoppingListRepo")
        .expect("the trait");
    let body = &source[start..];
    let body = &body[..body.find("\n}\n").expect("the trait's end")];
    body.lines()
        .filter_map(|line| line.trim().strip_prefix("async fn "))
        .map(|rest| rest[..rest.find('(').expect("a parameter list")].to_owned())
        .collect()
}

/// `source` split into its top-level functions: `(name, text)`. A function
/// starts at a line that begins with `fn`, `async fn` or their `pub` forms
/// and runs to the next one.
fn functions(source: &str) -> Vec<(String, String)> {
    let is_start = |line: &str| {
        [
            "fn ",
            "async fn ",
            "pub fn ",
            "pub async fn ",
            "pub(crate) fn ",
            "pub(crate) async fn ",
        ]
        .iter()
        .any(|p| line.starts_with(p))
    };
    let mut out: Vec<(String, String)> = Vec::new();
    for line in source.lines() {
        if is_start(line) {
            let after = &line[line.find("fn ").expect("fn") + 3..];
            let name: String = after
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            out.push((name, String::new()));
        }
        if let Some((_, text)) = out.last_mut() {
            text.push_str(line);
            text.push('\n');
        }
    }
    out
}

/// Functions in `sources` that call one of `calls` without announcing with
/// one of `announces`: `file::function`.
fn silent_writers(
    sources: &[(String, String)],
    calls: &[String],
    announces: &[&str],
) -> Vec<String> {
    let mut silent = Vec::new();
    for (file, source) in sources {
        for (name, text) in functions(source) {
            let writes = calls.iter().any(|call| text.contains(call.as_str()));
            if writes && !announces.iter().any(|a| text.contains(a)) {
                silent.push(format!("{file}::{name}"));
            }
        }
    }
    silent
}

fn calls(methods: &[&str]) -> Vec<String> {
    methods
        .iter()
        .map(|m| format!("ShoppingListRepo::{m}("))
        .collect()
}

#[test]
fn every_repo_method_is_classified() {
    let mut known: Vec<&str> = [READS, WRITES, UNANNOUNCED].concat();
    known.sort_unstable();
    let mut methods = repo_methods();
    methods.sort_unstable();
    assert_eq!(
        methods, known,
        "classify the new `ShoppingListRepo` method in READS or WRITES (RR-0049 constraint 3)"
    );
}

/// The guard itself: a write without an announcement is found, with one it
/// isn't, and a read never is.
#[test]
fn guard_finds_a_silent_write() {
    let source = "\
pub async fn quiet(state: &AppState) {
    let outcome = ShoppingListRepo::rename(&state.pool, user, list, name).await;
    respond(outcome)
}

async fn loud(state: &AppState) {
    let outcome = ShoppingListRepo::rename(&state.pool, user, list, name).await;
    announce(&state, &req, user, list, &outcome);
}

fn reader(state: &AppState) {
    ShoppingListRepo::get(&state.pool, user, list)
}
";
    let sources = [("x.rs".to_owned(), source.to_owned())];
    assert_eq!(
        silent_writers(&sources, &calls(WRITES), ANNOUNCES),
        ["x.rs::quiet"]
    );
}

/// RR-0049 constraint 3: no function in the web crate calls a
/// `ShoppingListRepo` write without announcing, and every write in WRITES
/// that a route uses is seen by the scan.
#[test]
fn every_list_write_announces() {
    let sources = source_files();
    let silent = silent_writers(&sources, &calls(WRITES), ANNOUNCES);
    assert!(
        silent.is_empty(),
        "these call a `ShoppingListRepo` write without `announce(…)` or \
         `state.live.changed(…)` (RR-0049 constraint 3): {silent:?}"
    );

    // The scan sees the calls: the routes use the `ShoppingListRepo::…`
    // form, and at least these functions are found writing.
    let writers: Vec<String> = sources
        .iter()
        .flat_map(|(file, source)| {
            functions(source)
                .into_iter()
                .filter(|(_, text)| calls(WRITES).iter().any(|c| text.contains(c.as_str())))
                .map(move |(name, _)| format!("{file}::{name}"))
        })
        .collect();
    for expected in [
        "routes/lists/mod.rs::rename",
        "routes/lists/mod.rs::delete",
        "routes/lists/archiving.rs::archive",
        "routes/lists/archiving.rs::unarchive",
        "routes/lists/items.rs::add_item",
        "routes/lists/items.rs::add_recipe",
        "routes/lists/items.rs::update_item",
        "routes/lists/items.rs::remove_item",
        "routes/lists/items.rs::restore_item",
        "routes/lists/store.rs::check_item",
        "routes/lists/store.rs::remove_from_shop",
        "routes/lists/shares.rs::share",
        "routes/lists/shares.rs::revoke",
        "routes/recipes/add_to_list.rs::add_recipe_to_list",
    ] {
        assert!(
            writers.iter().any(|w| w == expected),
            "{expected} in {writers:?}"
        );
    }
    // No route reaches a list write through the method-call form, which
    // the scan above wouldn't see.
    for (file, source) in &sources {
        if file.starts_with("routes/lists/") || file.starts_with("routes/recipes/") {
            for method in WRITES {
                assert!(
                    !source.contains(&format!(".pool.{method}(")),
                    "{file}: call `ShoppingListRepo::{method}(…)`, so the guard sees it"
                );
            }
        }
    }
}

/// RR-0049 sub-answer 6: the owner's category rename, move and delete
/// announce the owner's lists.
#[test]
fn category_changes_announce() {
    let sources: Vec<_> = source_files()
        .into_iter()
        .filter(|(file, _)| file == "routes/categories.rs")
        .collect();
    assert_eq!(sources.len(), 1);
    let calls: Vec<String> = CATEGORY_WRITES.iter().map(|c| (*c).to_owned()).collect();
    let silent = silent_writers(&sources, &calls, &[".live.owner_changed("]);
    assert!(silent.is_empty(), "{silent:?}");
    for name in ["rename", "delete", "move_category"] {
        let (_, text) = functions(&sources[0].1)
            .into_iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("categories::{name}"));
        assert!(text.contains(".live.owner_changed("), "{name}");
    }
}
