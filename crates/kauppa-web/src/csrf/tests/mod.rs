//! Tests for `csrf` (MB-0018, RR-0010), by area (RR-0009). The route
//! table, a signed-in user's data and its snapshot are shared here.
// Tests fail by panicking, and route templates (`/lists/{id}`) look like
// format strings.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::literal_string_with_formatting_args
)]

mod coverage;
mod cross_site;
mod logging;
mod origin;
mod same_origin;

use std::path::{Path, PathBuf};

use actix_web::http::Method;
use uuid::Uuid;

use kauppa_core::domain::ShareRole;
use kauppa_core::repo::{CategoryRepo, ProductRepo, RecipeRepo, ShoppingListRepo, UserRepo};

use crate::routes::test_support::{make_category, make_recipe_with, make_user};
use crate::state::AppState;

/// A route a handler attribute (`#[post("…")]` and friends) registers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub method: Method,
    /// Full path template, e.g. `/lists/{id}/items`.
    pub path: String,
    /// `<routes module>::<fn>`, as `routes::protected` names it.
    pub handler: String,
}

/// The crate's `src/`.
fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Every non-test `.rs` file under `src/`, as `(path relative to src, text)`:
/// skips `tests/` directories and `test_support.rs`.
pub fn source_files() -> Vec<(String, String)> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("read src dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n != "tests") {
                    walk(&path, out);
                }
            } else if path.extension().is_some_and(|e| e == "rs")
                && path.file_name().is_some_and(|n| n != "test_support.rs")
            {
                out.push(path);
            }
        }
    }
    let src = src_dir();
    let mut paths = Vec::new();
    walk(&src, &mut paths);
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let rel = p.strip_prefix(&src).expect("under src");
            let rel = rel.to_string_lossy().replace('\\', "/");
            (rel, std::fs::read_to_string(&p).expect("read source"))
        })
        .collect()
}

/// actix-web's route attribute macros and the method each registers
/// (`route` names its methods in `method = "…"` arguments).
const ROUTE_MACROS: &[&str] = &[
    "get", "post", "put", "patch", "delete", "head", "options", "trace", "connect", "route",
];

/// A handler attribute on `line`: `(macro, path, methods)`.
fn parse_route_attr(line: &str) -> Option<(String, String, Vec<Method>)> {
    let inner = line.trim().strip_prefix("#[")?;
    let inner = inner.strip_prefix("actix_web::").unwrap_or(inner);
    let (name, args) = inner.split_once('(')?;
    if !ROUTE_MACROS.contains(&name) {
        return None;
    }
    let path = args.split('"').nth(1)?.to_owned();
    let methods = if name == "route" {
        args.split("method")
            .skip(1)
            .filter_map(|m| m.split('"').nth(1))
            .map(|m| Method::from_bytes(m.as_bytes()).expect("method"))
            .collect()
    } else {
        vec![Method::from_bytes(name.to_ascii_uppercase().as_bytes()).expect("method")]
    };
    Some((name.to_owned(), path, methods))
}

/// Every handler attribute in the source, as `(handler, path relative to
/// its scope, method)`; the handler is `<module>::<fn>`, the module being
/// the file's top-level module under `routes/` (or the file itself outside).
pub fn handler_attrs() -> Vec<(String, String, Method)> {
    let mut found = Vec::new();
    for (file, text) in source_files() {
        let module = file
            .strip_prefix("routes/")
            .unwrap_or(&file)
            .split('/')
            .next()
            .expect("module")
            .trim_end_matches(".rs")
            .to_owned();
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let Some((_, path, methods)) = parse_route_attr(line) else {
                continue;
            };
            let name = lines[i + 1..]
                .iter()
                .find_map(|l| {
                    let l = l.trim();
                    let rest = l.strip_prefix("pub ").unwrap_or(l);
                    let rest = rest.strip_prefix("async ").unwrap_or(rest);
                    rest.strip_prefix("fn ")
                })
                .and_then(|rest| rest.split(['(', '<']).next())
                .unwrap_or_else(|| panic!("{file}:{}: no fn after the attribute", i + 1));
            for method in methods {
                found.push((format!("{module}::{name}"), path.clone(), method));
            }
        }
    }
    found
}

/// `routes::protected`'s registrations, as `(handler, scope prefix)`, read
/// from `routes/mod.rs` (`.service(m::f)` lines, inside `web::scope("…")`
/// blocks).
pub fn protected_services() -> Vec<(String, String)> {
    let text = std::fs::read_to_string(src_dir().join("routes/mod.rs")).expect("routes/mod.rs");
    let start = text.find("fn protected(").expect("fn protected");
    let body = &text[start..];
    let body = &body[..body.find("\n}\n").expect("end of fn protected")];
    let mut prefixes: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for line in body.lines().skip(1) {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.split("web::scope(\"").nth(1) {
            prefixes.push(rest.split('"').next().expect("prefix").to_owned());
        } else if let Some(rest) = trimmed.split(".service(").nth(1) {
            let name = rest.trim_end_matches([')', ',', ';']);
            if !name.is_empty() {
                out.push((
                    name.to_owned(),
                    prefixes.last().cloned().unwrap_or_default(),
                ));
            }
        } else if trimmed.starts_with(')') {
            prefixes.pop();
        }
    }
    out
}

/// Every route the app registers, with full paths.
pub fn routes() -> Vec<Route> {
    let services = protected_services();
    handler_attrs()
        .into_iter()
        .map(|(handler, path, method)| {
            let Some((_, prefix)) = services.iter().find(|(h, _)| *h == handler) else {
                panic!("{handler} isn't registered in routes::protected");
            };
            Route {
                method,
                path: format!("{prefix}{path}"),
                handler,
            }
        })
        .collect()
}

/// Every route that accepts a non-safe method.
pub fn write_routes() -> Vec<Route> {
    let writes: Vec<_> = routes()
        .into_iter()
        .filter(|r| !crate::csrf::is_safe(&r.method))
        .collect();
    // MB-0018's baseline: 34 POST routes. Fewer means the scan broke.
    assert!(
        writes.len() >= 34,
        "found only {} write routes",
        writes.len()
    );
    writes
}

/// The emails the fixture and the forged bodies use.
pub const OWNER: &str = "owner@example.com";
pub const VIEWER: &str = "viewer@example.com";
pub const EDITOR: &str = "editor@example.com";
pub const NEWCOMER: &str = "newcomer@example.com";

/// A signed-in victim's data: something for every write route to change.
#[derive(Debug, Clone, Copy)]
pub struct World {
    pub owner: Uuid,
    pub viewer: Uuid,
    pub category: Uuid,
    pub product: Uuid,
    pub recipe: Uuid,
    pub ingredient: Uuid,
    pub list: Uuid,
    pub item: Uuid,
    pub removed: Uuid,
    pub share: Uuid,
}

pub async fn world(state: &AppState) -> World {
    let owner = make_user(state, OWNER).await;
    let viewer = make_user(state, VIEWER).await;
    make_user(state, EDITOR).await;
    let category = make_category(state, owner, "Kylmä").await;
    make_category(state, owner, "Hevi").await;
    let product = ProductRepo::create(&state.pool, owner, "Maito", category)
        .await
        .expect("favorite")
        .id;
    ProductRepo::create(&state.pool, owner, "Jogurtti", category)
        .await
        .expect("favorite");
    let recipe = make_recipe_with(
        state,
        owner,
        "Puuro",
        category,
        &[("Kaura", Some("2 dl")), ("Suola", None)],
    )
    .await;
    let ingredient = RecipeRepo::get(&state.pool, owner, recipe)
        .await
        .expect("recipe")
        .ingredients[0]
        .id;
    let list = ShoppingListRepo::create(&state.pool, owner, "Kauppa")
        .await
        .expect("list")
        .id;
    let item = ShoppingListRepo::add_item(&state.pool, owner, list, "Leipä", None)
        .await
        .expect("item")
        .id;
    ShoppingListRepo::add_item(&state.pool, owner, list, "Munat", None)
        .await
        .expect("item");
    let removed = ShoppingListRepo::add_item(&state.pool, owner, list, "Sokeri", None)
        .await
        .expect("item")
        .id;
    ShoppingListRepo::remove_item(&state.pool, owner, list, removed)
        .await
        .expect("remove");
    let share = ShoppingListRepo::share(&state.pool, owner, list, VIEWER, ShareRole::View)
        .await
        .expect("share")
        .expect("viewer matched")
        .id;
    World {
        owner,
        viewer,
        category,
        product,
        recipe,
        ingredient,
        list,
        item,
        removed,
        share,
    }
}

impl World {
    /// `route`'s path with its parameters set to this world's ids.
    pub fn uri(&self, route: &Route) -> String {
        let scope = route.path.split('/').nth(1).unwrap_or_default();
        let id = match scope {
            "lists" => self.list,
            "recipes" => self.recipe,
            "categories" => self.category,
            "favorites" => self.product,
            _ => Uuid::nil(),
        };
        let item = if route.path.ends_with("/restore") {
            self.removed
        } else {
            self.item
        };
        route
            .path
            .replace("{id}", &id.to_string())
            .replace("{item}", &item.to_string())
            .replace("{share}", &self.share.to_string())
            .replace("{ingredient}", &self.ingredient.to_string())
    }

    /// A form a handler on `route` would accept, so only the check stands
    /// between the request and a change: every field any write form reads.
    pub fn form(&self, route: &Route) -> Vec<(&'static str, String)> {
        let email = match route.path.as_str() {
            "/auth/login" => OWNER,
            "/auth/register" => NEWCOMER,
            _ => EDITOR,
        };
        vec![
            ("email", email.to_owned()),
            ("password", crate::routes::test_support::PASSWORD.to_owned()),
            ("display_name", "Mallory".to_owned()),
            ("name", "Hijacked".to_owned()),
            ("title", "Hijacked".to_owned()),
            ("text", "Hijacked".to_owned()),
            ("instructions", "Hijacked".to_owned()),
            ("servings", "9".to_owned()),
            ("quantity", "9 kg".to_owned()),
            ("role", "edit".to_owned()),
            ("checked", "true".to_owned()),
            ("dir", "down".to_owned()),
            ("multiplier", "2".to_owned()),
            ("category_id", self.category.to_string()),
            ("product_id", self.product.to_string()),
            ("recipe_id", self.recipe.to_string()),
            ("list_id", self.list.to_string()),
        ]
    }

    /// Everything the write routes could change, as one comparable string.
    pub async fn snapshot(&self, state: &AppState) -> String {
        let pool = &state.pool;
        let lists = ShoppingListRepo::list_for_owner(pool, self.owner).await;
        let viewer_lists = ShoppingListRepo::shared_with(pool, self.viewer).await;
        let active = ShoppingListRepo::items(pool, self.owner, self.list, false).await;
        let removed = ShoppingListRepo::items(pool, self.owner, self.list, true).await;
        let shares = ShoppingListRepo::shares(pool, self.owner, self.list).await;
        let recipes = RecipeRepo::list_for_owner(pool, self.owner).await;
        let recipe = RecipeRepo::get(pool, self.owner, self.recipe).await;
        let categories = CategoryRepo::list_for_owner(pool, self.owner).await;
        let favorites = ProductRepo::list_for_owner(pool, self.owner).await;
        let hidden = ProductRepo::list_hidden_with_usage(pool, self.owner).await;
        let newcomer = UserRepo::find_by_email(pool, NEWCOMER)
            .await
            .map(|u| u.is_some());
        format!(
            "{lists:?}\n{viewer_lists:?}\n{active:?}\n{removed:?}\n{shares:?}\n{recipes:?}\n\
             {recipe:?}\n{categories:?}\n{favorites:?}\n{hidden:?}\n{newcomer:?}"
        )
    }
}
