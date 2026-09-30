// `SqlxModel`, `SqlxNewModel` and `SqlxUpdatableModel`: the `Model`, `NewModel` and `UpdatableModel`
// implementations querying a table with sqlx, their SQL is written at compile time from the struct fields.
// The types of the queries and of the application state are read from the `http_*` attribute
// of the struct, the rest from `#[sqlx_model(...)]`.
use darling::FromMeta;
use proc_macro2::TokenStream;
use quote::quote;
use syn::ext::IdentExt;

use crate::{HttpCreateDeriveParams, HttpFindListDeleteDeriveParams, HttpUpdateDeriveParams};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Database {
    Sqlite,
    Postgres,
    Mysql,
}

impl Database {
    fn parse(name: &str) -> Option<Database> {
        match name {
            "sqlite" => Some(Database::Sqlite),
            "postgres" => Some(Database::Postgres),
            "mysql" => Some(Database::Mysql),
            _ => None,
        }
    }

    // `$1, $2...` for SQLite and PostgreSQL, `?` for MySQL, numbered from `from`
    fn placeholders(self, from: usize, count: usize) -> Vec<String> {
        (from..from + count)
            .map(|i| if self == Database::Mysql { "?".to_string() } else { format!("${}", i) })
            .collect()
    }

    // MySQL has no RETURNING
    fn returning(self) -> bool {
        self != Database::Mysql
    }
}

#[derive(Debug, FromMeta)]
struct SqlxModelArgs {
    /// `sqlite`, `postgres` or `mysql`
    database: String,
    /// Defaults to the lowercase name of the model
    #[darling(default)]
    table: Option<String>,
    /// The model returned by `save` (only for `SqlxNewModel`)
    #[darling(default)]
    model: Option<syn::Path>,
    /// Field of the application state holding the sqlx pool
    #[darling(default)]
    pool: Option<String>,
    /// `created_at` and `updated_at` are set on insert, `updated_at` on update
    #[darling(default)]
    timestamps: bool,
    /// delete sets `deleted_at`, and find, list and update skip the deleted rows
    #[darling(default)]
    soft_delete: bool,
    #[darling(default)]
    default_limit: Option<i64>,
    #[darling(default)]
    max_limit: Option<i64>,
}

const DEFAULT_LIMIT: i64 = 20;
const MAX_LIMIT: i64 = 100;
const TIMESTAMP_COLUMNS: [&str; 3] = ["created_at", "updated_at", "deleted_at"];

struct Config {
    database: Database,
    table: String,
    pool: syn::Ident,
    timestamps: bool,
    soft_delete: bool,
    model: Option<syn::Path>,
    default_limit: i64,
    max_limit: i64,
}

fn find_attr<'a>(ast: &'a syn::DeriveInput, name: &str) -> Option<&'a syn::Attribute> {
    ast.attrs.iter().find(|a| a.path.is_ident(name))
}

fn parse_http_attr<T: syn::parse::Parse>(ast: &syn::DeriveInput, name: &str, derive: &str) -> syn::Result<T> {
    let attr = find_attr(ast, name).ok_or_else(|| {
        syn::Error::new_spanned(&ast.ident, format!("{} requires the #[{}(...)] attribute", derive, name))
    })?;
    syn::parse2(attr.tokens.clone())
}

// `default_table` gives the table when `table` is not set
fn parse_config(ast: &syn::DeriveInput, derive: &str, default_table: impl FnOnce(&SqlxModelArgs) -> Option<String>) -> syn::Result<Config> {
    let attr = find_attr(ast, "sqlx_model").ok_or_else(|| {
        syn::Error::new_spanned(&ast.ident, format!("{} requires the #[sqlx_model(database = \"...\")] attribute", derive))
    })?;
    let meta = attr.parse_meta()?;
    let args = SqlxModelArgs::from_meta(&meta).map_err(|e| syn::Error::new_spanned(&meta, e.to_string()))?;
    let database = Database::parse(&args.database).ok_or_else(|| {
        syn::Error::new_spanned(&meta, format!("unknown database `{}`, use sqlite, postgres or mysql", args.database))
    })?;
    let table = match args.table.clone().or_else(|| default_table(&args)) {
        Some(table) => table,
        None => return Err(syn::Error::new_spanned(&meta, format!("{} requires `table` or `model`", derive))),
    };
    let pool = syn::parse_str(args.pool.as_deref().unwrap_or("pool"))?;
    Ok(Config {
        database,
        table,
        pool,
        timestamps: args.timestamps,
        soft_delete: args.soft_delete,
        model: args.model,
        default_limit: args.default_limit.unwrap_or(DEFAULT_LIMIT),
        max_limit: args.max_limit.unwrap_or(MAX_LIMIT),
    })
}

// Field identifiers of a struct with named fields
fn named_fields<'a>(ast: &'a syn::DeriveInput, derive: &str) -> syn::Result<Vec<&'a syn::Ident>> {
    match &ast.data {
        syn::Data::Struct(syn::DataStruct { fields: syn::Fields::Named(fields), .. }) => {
            Ok(fields.named.iter().filter_map(|f| f.ident.as_ref()).collect())
        }
        _ => Err(syn::Error::new_spanned(&ast.ident, format!("{} only supports structs with named fields", derive))),
    }
}

// Column of a field, `r#type` is the `type` column
fn column(field: &syn::Ident) -> String {
    field.unraw().to_string()
}

fn require_id(ast: &syn::DeriveInput, fields: &[&syn::Ident], derive: &str) -> syn::Result<()> {
    if fields.iter().any(|f| *f == "id") {
        Ok(())
    } else {
        Err(syn::Error::new_spanned(&ast.ident, format!("{} requires an `id` field, the primary key", derive)))
    }
}

fn not_deleted(config: &Config) -> &'static str {
    if config.soft_delete { " AND deleted_at IS NULL" } else { "" }
}

fn now() -> TokenStream {
    quote! { ::actix_restful::__private::chrono::Utc::now() }
}

pub fn impl_sqlx_model(ast: &syn::DeriveInput) -> syn::Result<TokenStream> {
    const DERIVE: &str = "SqlxModel";
    let HttpFindListDeleteDeriveParams(id, find_query, list_query, delete_query, app_state) =
        parse_http_attr(ast, "http_find_list_delete", DERIVE)?;
    let name = &ast.ident;
    let config = parse_config(ast, DERIVE, |_| Some(name.to_string().to_lowercase()))?;
    let fields = named_fields(ast, DERIVE)?;
    require_id(ast, &fields, DERIVE)?;
    let db = config.database;
    let (table, pool) = (&config.table, &config.pool);
    let p = db.placeholders(1, 3);

    let find_sql = format!("SELECT * FROM {} WHERE id = {}{}", table, p[0], not_deleted(&config));
    let list_sql = format!(
        "SELECT * FROM {}{} ORDER BY id LIMIT {} OFFSET {}",
        table,
        if config.soft_delete { " WHERE deleted_at IS NULL" } else { "" },
        p[0],
        p[1]
    );
    let select_by_id = format!("SELECT * FROM {} WHERE id = {}", table, p[0]);
    let now = now();
    let delete_body = match (config.soft_delete, db.returning()) {
        (true, true) => {
            let sql = format!("UPDATE {} SET deleted_at = {} WHERE id = {}{} RETURNING *", table, p[0], p[1], not_deleted(&config));
            quote! {
                let model = ::actix_restful::__private::sqlx::query_as::<_, #name>(#sql)
                    .bind(#now)
                    .bind(&self.id)
                    .fetch_one(&state.#pool)
                    .await?;
            }
        }
        // the row is selected after its update, which fails when it was already deleted
        (true, false) => {
            let sql = format!("UPDATE {} SET deleted_at = {} WHERE id = {}{}", table, p[0], p[1], not_deleted(&config));
            quote! {
                let result = ::actix_restful::__private::sqlx::query(#sql)
                    .bind(#now)
                    .bind(&self.id)
                    .execute(&state.#pool)
                    .await?;
                if result.rows_affected() == 0 {
                    return Err(::actix_restful::anyhow::anyhow!("ENTITY_NOT_FOUND"));
                }
                let model = ::actix_restful::__private::sqlx::query_as::<_, #name>(#select_by_id)
                    .bind(&self.id)
                    .fetch_one(&state.#pool)
                    .await?;
            }
        }
        (false, true) => {
            let sql = format!("DELETE FROM {} WHERE id = {} RETURNING *", table, p[0]);
            quote! {
                let model = ::actix_restful::__private::sqlx::query_as::<_, #name>(#sql)
                    .bind(&self.id)
                    .fetch_one(&state.#pool)
                    .await?;
            }
        }
        // the row is selected before its deletion
        (false, false) => {
            let sql = format!("DELETE FROM {} WHERE id = {}", table, p[0]);
            quote! {
                let model = ::actix_restful::__private::sqlx::query_as::<_, #name>(#select_by_id)
                    .bind(&self.id)
                    .fetch_one(&state.#pool)
                    .await?;
                ::actix_restful::__private::sqlx::query(#sql)
                    .bind(&self.id)
                    .execute(&state.#pool)
                    .await?;
            }
        }
    };
    let (default_limit, max_limit) = (config.default_limit, config.max_limit);
    Ok(quote! {
        #[::actix_restful::__private::async_trait]
        impl ::actix_restful::Model<#id, #find_query, #list_query, ::std::vec::Vec<#name>, #delete_query, #name, #app_state> for #name {
            async fn find(id: #id, _query: &#find_query, state: &#app_state) -> ::actix_restful::anyhow::Result<::std::boxed::Box<#name>> {
                let model = ::actix_restful::__private::sqlx::query_as::<_, #name>(#find_sql)
                    .bind(id)
                    .fetch_one(&state.#pool)
                    .await?;
                Ok(::std::boxed::Box::new(model))
            }
            async fn list(query: &#list_query, state: &#app_state) -> ::actix_restful::anyhow::Result<::std::vec::Vec<#name>> {
                let offset = query.offset.unwrap_or(0) as i64;
                let limit = query.limit.map_or(#default_limit, |l| (l as i64).min(#max_limit));
                let models = ::actix_restful::__private::sqlx::query_as::<_, #name>(#list_sql)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(&state.#pool)
                    .await?;
                Ok(models)
            }
            async fn delete(self: Self, _query: &#delete_query, state: &#app_state) -> ::actix_restful::anyhow::Result<#name> {
                #delete_body
                Ok(model)
            }
        }
    })
}

pub fn impl_sqlx_new_model(ast: &syn::DeriveInput) -> syn::Result<TokenStream> {
    const DERIVE: &str = "SqlxNewModel";
    let HttpCreateDeriveParams(save_query, app_state) = parse_http_attr(ast, "http_create", DERIVE)?;
    let name = &ast.ident;
    let config = parse_config(ast, DERIVE, |args| {
        args.model.as_ref().and_then(|m| m.segments.last()).map(|s| s.ident.to_string().to_lowercase())
    })?;
    let model = config.model.as_ref().ok_or_else(|| {
        syn::Error::new_spanned(name, "SqlxNewModel requires `model`, the model returned by `save`, in #[sqlx_model(...)]")
    })?;
    let fields = named_fields(ast, DERIVE)?;
    let db = config.database;
    let (table, pool) = (&config.table, &config.pool);

    let mut columns: Vec<String> = fields.iter().map(|f| column(f)).collect();
    let mut binds: Vec<TokenStream> = fields.iter().map(|f| quote! { .bind(&self.#f) }).collect();
    // created_at and updated_at get the same value
    if config.timestamps {
        columns.extend(["created_at".to_string(), "updated_at".to_string()]);
        binds.extend([quote! { .bind(now) }, quote! { .bind(now) }]);
    }
    let insert = if columns.is_empty() {
        if db == Database::Mysql {
            format!("INSERT INTO {} () VALUES ()", table)
        } else {
            format!("INSERT INTO {} DEFAULT VALUES", table)
        }
    } else {
        format!(
            "INSERT INTO {} ({}) VALUES ({})",
            table,
            columns.join(", "),
            db.placeholders(1, columns.len()).join(", ")
        )
    };
    let now = if config.timestamps {
        let now = self::now();
        quote! { let now = #now; }
    } else {
        quote! {}
    };
    let body = if db.returning() {
        let sql = format!("{} RETURNING *", insert);
        quote! {
            let model = ::actix_restful::__private::sqlx::query_as::<_, #model>(#sql)
                #(#binds)*
                .fetch_one(&state.#pool)
                .await?;
        }
    } else {
        let select = format!("SELECT * FROM {} WHERE id = ?", table);
        quote! {
            let result = ::actix_restful::__private::sqlx::query(#insert)
                #(#binds)*
                .execute(&state.#pool)
                .await?;
            let model = ::actix_restful::__private::sqlx::query_as::<_, #model>(#select)
                .bind(result.last_insert_id())
                .fetch_one(&state.#pool)
                .await?;
        }
    };
    Ok(quote! {
        #[::actix_restful::__private::async_trait]
        impl ::actix_restful::NewModel<#model, #save_query, #app_state> for #name {
            async fn save(self: Self, _query: &#save_query, state: &#app_state) -> ::actix_restful::anyhow::Result<#model> {
                #now
                #body
                Ok(model)
            }
        }
    })
}

pub fn impl_sqlx_updatable_model(ast: &syn::DeriveInput) -> syn::Result<TokenStream> {
    const DERIVE: &str = "SqlxUpdatableModel";
    let HttpUpdateDeriveParams(_id, update_query, output, _find_query, app_state) = parse_http_attr(ast, "http_update", DERIVE)?;
    let name = &ast.ident;
    let config = parse_config(ast, DERIVE, |_| Some(output.to_string().to_lowercase()))?;
    let fields = named_fields(ast, DERIVE)?;
    require_id(ast, &fields, DERIVE)?;
    let db = config.database;
    let (table, pool) = (&config.table, &config.pool);

    // with timestamps, the timestamp columns are not taken from the payload
    let set: Vec<&&syn::Ident> = fields
        .iter()
        .filter(|f| **f != "id" && !(config.timestamps && TIMESTAMP_COLUMNS.contains(&column(f).as_str())))
        .collect();
    let mut columns: Vec<String> = set.iter().map(|f| column(f)).collect();
    let mut binds: Vec<TokenStream> = set.iter().map(|f| quote! { .bind(&self.#f) }).collect();
    if config.timestamps {
        columns.push("updated_at".to_string());
        let now = now();
        binds.push(quote! { .bind(#now) });
    }
    if columns.is_empty() {
        return Err(syn::Error::new_spanned(name, "SqlxUpdatableModel requires a field to update besides `id`"));
    }
    let mut placeholders = db.placeholders(1, columns.len() + 1);
    let id_placeholder = placeholders.pop().unwrap_or_default();
    let assignments: Vec<String> = columns.iter().zip(&placeholders).map(|(c, p)| format!("{} = {}", c, p)).collect();
    let update = format!(
        "UPDATE {} SET {} WHERE id = {}{}",
        table,
        assignments.join(", "),
        id_placeholder,
        not_deleted(&config)
    );
    // the columns of the updatable struct
    let returned: Vec<String> = fields.iter().map(|f| column(f)).collect();
    let body = if db.returning() {
        let sql = format!("{} RETURNING {}", update, returned.join(", "));
        quote! {
            let model = ::actix_restful::__private::sqlx::query_as::<_, #name>(#sql)
                #(#binds)*
                .bind(&self.id)
                .fetch_one(&state.#pool)
                .await?;
        }
    } else {
        let select = format!("SELECT {} FROM {} WHERE id = ?{}", returned.join(", "), table, not_deleted(&config));
        quote! {
            ::actix_restful::__private::sqlx::query(#update)
                #(#binds)*
                .bind(&self.id)
                .execute(&state.#pool)
                .await?;
            let model = ::actix_restful::__private::sqlx::query_as::<_, #name>(#select)
                .bind(&self.id)
                .fetch_one(&state.#pool)
                .await?;
        }
    };
    Ok(quote! {
        #[::actix_restful::__private::async_trait]
        impl ::actix_restful::UpdatableModel<#name, #update_query, #app_state> for #name {
            async fn update(self: Self, _query: &#update_query, state: &#app_state) -> ::actix_restful::anyhow::Result<#name> {
                #body
                Ok(model)
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expand(f: fn(&syn::DeriveInput) -> syn::Result<TokenStream>, ast: syn::DeriveInput) -> String {
        f(&ast).map(|t| t.to_string()).unwrap_or_else(|e| format!("error: {}", e))
    }

    #[test]
    fn model_queries_the_lowercase_table() {
        let out = expand(impl_sqlx_model, syn::parse_quote! {
            #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
            #[sqlx_model(database = "postgres")]
            struct Project { id: Id, name: String }
        });
        assert!(out.contains("\"SELECT * FROM project WHERE id = $1\""), "{}", out);
        assert!(out.contains("\"SELECT * FROM project ORDER BY id LIMIT $1 OFFSET $2\""), "{}", out);
        assert!(out.contains("\"DELETE FROM project WHERE id = $1 RETURNING *\""), "{}", out);
    }

    #[test]
    fn soft_delete_skips_the_deleted_rows() {
        let out = expand(impl_sqlx_model, syn::parse_quote! {
            #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
            #[sqlx_model(database = "sqlite", table = "projects", soft_delete)]
            struct Project { id: Id, deleted_at: Option<DateTime<Utc>> }
        });
        assert!(out.contains("\"SELECT * FROM projects WHERE id = $1 AND deleted_at IS NULL\""), "{}", out);
        assert!(out.contains("\"SELECT * FROM projects WHERE deleted_at IS NULL ORDER BY id LIMIT $1 OFFSET $2\""), "{}", out);
        assert!(out.contains("\"UPDATE projects SET deleted_at = $1 WHERE id = $2 AND deleted_at IS NULL RETURNING *\""), "{}", out);
    }

    #[test]
    fn mysql_model_selects_instead_of_returning() {
        let out = expand(impl_sqlx_model, syn::parse_quote! {
            #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
            #[sqlx_model(database = "mysql", pool = "db")]
            struct Project { id: Id }
        });
        assert!(out.contains("\"SELECT * FROM project ORDER BY id LIMIT ? OFFSET ?\""), "{}", out);
        assert!(out.contains("\"DELETE FROM project WHERE id = ?\""), "{}", out);
        assert!(!out.contains("RETURNING"), "{}", out);
        assert!(out.contains("state . db"), "{}", out);
    }

    #[test]
    fn model_requires_an_id_and_its_attributes() {
        let no_id = expand(impl_sqlx_model, syn::parse_quote! {
            #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
            #[sqlx_model(database = "sqlite")]
            struct Project { name: String }
        });
        assert!(no_id.starts_with("error: SqlxModel requires an `id` field"), "{}", no_id);
        let no_http = expand(impl_sqlx_model, syn::parse_quote! {
            #[sqlx_model(database = "sqlite")]
            struct Project { id: Id }
        });
        assert!(no_http.contains("#[http_find_list_delete(...)]"), "{}", no_http);
        let unknown = expand(impl_sqlx_model, syn::parse_quote! {
            #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
            #[sqlx_model(database = "oracle")]
            struct Project { id: Id }
        });
        assert!(unknown.contains("unknown database `oracle`"), "{}", unknown);
    }

    #[test]
    fn new_model_inserts_its_fields_and_the_timestamps() {
        let out = expand(impl_sqlx_new_model, syn::parse_quote! {
            #[http_create(SaveQuery, AppState)]
            #[sqlx_model(database = "postgres", model = "Project", timestamps)]
            struct NewProject { name: String, r#type: i32 }
        });
        assert!(
            out.contains("\"INSERT INTO project (name, type, created_at, updated_at) VALUES ($1, $2, $3, $4) RETURNING *\""),
            "{}",
            out
        );
        assert!(out.contains("NewModel < Project , SaveQuery , AppState >"), "{}", out);
    }

    #[test]
    fn new_model_without_columns_inserts_the_defaults() {
        let sqlite = expand(impl_sqlx_new_model, syn::parse_quote! {
            #[http_create(SaveQuery, AppState)]
            #[sqlx_model(database = "sqlite", model = "Project")]
            struct NewProject {}
        });
        assert!(sqlite.contains("\"INSERT INTO project DEFAULT VALUES RETURNING *\""), "{}", sqlite);
        let mysql = expand(impl_sqlx_new_model, syn::parse_quote! {
            #[http_create(SaveQuery, AppState)]
            #[sqlx_model(database = "mysql", model = "Project")]
            struct NewProject {}
        });
        assert!(mysql.contains("\"INSERT INTO project () VALUES ()\""), "{}", mysql);
        assert!(mysql.contains("last_insert_id"), "{}", mysql);
    }

    #[test]
    fn new_model_requires_its_model() {
        let out = expand(impl_sqlx_new_model, syn::parse_quote! {
            #[http_create(SaveQuery, AppState)]
            #[sqlx_model(database = "sqlite", table = "project")]
            struct NewProject { name: String }
        });
        assert!(out.starts_with("error: SqlxNewModel requires `model`"), "{}", out);
    }

    #[test]
    fn updatable_model_sets_its_fields_and_returns_them() {
        let out = expand(impl_sqlx_updatable_model, syn::parse_quote! {
            #[http_update(Id, UpdateQuery, Project, FindQuery, AppState)]
            #[sqlx_model(database = "postgres", timestamps, soft_delete)]
            struct UpdatableProject { id: Id, name: String, updated_at: Option<DateTime<Utc>> }
        });
        assert!(
            out.contains("\"UPDATE project SET name = $1, updated_at = $2 WHERE id = $3 AND deleted_at IS NULL RETURNING id, name, updated_at\""),
            "{}",
            out
        );
    }

    #[test]
    fn updatable_model_requires_a_column() {
        let out = expand(impl_sqlx_updatable_model, syn::parse_quote! {
            #[http_update(Id, UpdateQuery, Project, FindQuery, AppState)]
            #[sqlx_model(database = "sqlite")]
            struct UpdatableProject { id: Id }
        });
        assert!(out.starts_with("error: SqlxUpdatableModel requires a field to update"), "{}", out);
    }
}
