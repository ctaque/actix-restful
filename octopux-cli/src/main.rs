use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use sqlx::TypeInfo;
use structopt::StructOpt;
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write, Error};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

// Whether the messages are colored, enabled by `main` when writing to a terminal and NO_COLOR is not set,
// so the tests and the piped output get plain text
static COLOR: AtomicBool = AtomicBool::new(false);

// Wraps `text` in the ANSI `code` style when colors are enabled,
// the style is restored after the resets of the spans already painted in `text`
fn paint(code: &str, text: &str) -> String {
    if COLOR.load(Ordering::Relaxed) {
        let start = format!("\x1b[{}m", code);
        format!("{}{}\x1b[0m", start, text.replace("\x1b[0m", &format!("\x1b[0m{}", start)))
    } else {
        text.to_string()
    }
}

fn bold(text: &str) -> String { paint("1", text) }
fn dim(text: &str) -> String { paint("2", text) }
fn red(text: &str) -> String { paint("31", text) }
fn green(text: &str) -> String { paint("32", text) }
fn yellow(text: &str) -> String { paint("33", text) }
fn cyan(text: &str) -> String { paint("36", text) }
fn magenta(text: &str) -> String { paint("35", text) }

// Colors the `code` spans of a message, backticks included
fn highlight(message: &str) -> String {
    message
        .split('`')
        .enumerate()
        .map(|(i, part)| if i % 2 == 1 { cyan(&format!("`{}`", part)) } else { part.to_string() })
        .collect()
}

fn success(message: &str) -> String {
    format!("{} {}", green("✔"), highlight(message))
}

fn failure(message: &str) -> String {
    format!("{} {}", red("✘"), highlight(message))
}

fn warning(message: &str) -> String {
    format!("{} {}", yellow("!"), highlight(message))
}

#[derive(Debug, StructOpt)]
#[structopt(name = "octopux")]
pub struct Cli {
    /// Generates src/main.rs and src/helpers.rs of an actix server, if src/helpers.rs does not exist,
    /// prompting before overwriting an existing src/main.rs, then prompts for adding their dependencies to Cargo.toml with `cargo add`
    #[structopt(long = "bootstrap")]
    bootstrap: bool,
    /// With --bootstrap, serves the routes on an apistos app documented with OpenAPI and Swagger UI,
    /// to mount models generated with --openapi, instead of a plain actix app
    #[structopt(long = "openapi", requires = "bootstrap")]
    openapi: bool,
    #[structopt(subcommand)]
    cmd: Option<Opt>,
}

#[derive(Debug, StructOpt)]
pub enum Opt {
    #[structopt(name = "generate-model")]
    GenerateModel {
        #[structopt(short = "n", long = "name")]
        name: String,
        /// Derives JsonSchema and ApiComponent on the model types, for `gen_documented_endpoint!`
        #[structopt(long = "openapi")]
        openapi: bool,
        /// Interactively prompts for the fields of the model, added to the model, creatable and updatable structs
        #[structopt(long = "fields")]
        fields: bool,
        /// Derives SqlxModel, SqlxNewModel and SqlxUpdatableModel (octopux `sqlx` feature),
        /// which query the `pool` of the AppState, instead of leaving the model functions to fill, requires --fields
        #[structopt(long = "sqlx", requires = "fields")]
        sqlx: bool,
        /// Creates the migration of the model table in the migrations folder next to src, requires --fields
        #[structopt(long = "migration", requires = "fields")]
        migration: bool,
        /// Asks, for each field, for the table and the column it references, among the tables of the DATABASE_URL database
        /// (the tables created by the migrations when it is not set) and the model itself, the migration declares the foreign keys,
        /// requires --migration
        #[structopt(long = "foreign-keys", requires = "migration")]
        foreign_keys: bool,
        /// Asks, for each field, whether its column is unique, the migration declares the unique constraints,
        /// requires --migration
        #[structopt(long = "unique", requires = "migration")]
        unique: bool,
        /// Targets SQLite with the sqlx queries and the migration (the default)
        #[structopt(long = "sqlite", conflicts_with_all = &["postgres", "mysql"])]
        sqlite: bool,
        /// Targets PostgreSQL with the sqlx queries and the migration
        #[structopt(long = "postgres", conflicts_with = "mysql")]
        postgres: bool,
        /// Targets MySQL with the sqlx queries and the migration
        #[structopt(long = "mysql")]
        mysql: bool,
        /// Adds `created_at`, `updated_at` and `deleted_at` columns to the model and the migration,
        /// with --sqlx, they are set by the queries, and `delete` becomes a soft delete setting `deleted_at`
        #[structopt(long = "timestamps")]
        timestamps: bool,
        /// Overwrites the model file when it already exists, the code written in it is lost
        #[structopt(long = "force")]
        force: bool,
    },
    /// Generates a has-many relation, served on `GET /{parent}/{id}/{relation}` and paginated
    #[structopt(name = "generate-relation")]
    GenerateRelation {
        /// The parent model, e.g. `Project`
        #[structopt(long = "parent")]
        parent: String,
        /// The child model, e.g. `Book`
        #[structopt(long = "child")]
        child: String,
        /// The last segment of the route, the plural of the child by default (`books`)
        #[structopt(long = "name")]
        name: Option<String>,
        /// The column referencing the parent, in the child table or in the --through table,
        /// `{parent}_id` by default (`project_id`)
        #[structopt(long = "foreign-key")]
        foreign_key: Option<String>,
        /// The join model of a many-to-many relation, e.g. `ProjectCategory`
        #[structopt(long = "through")]
        through: Option<String>,
        /// The column of the --through table referencing the child, `{child}_id` by default (`category_id`)
        #[structopt(long = "child-key", requires = "through")]
        child_key: Option<String>,
        /// Derives JsonSchema and ApiComponent on the query, and documents the route
        #[structopt(long = "openapi")]
        openapi: bool,
        /// Fills the relation with sqlx queries on the `pool` of the AppState
        #[structopt(long = "sqlx")]
        sqlx: bool,
        /// Creates the migration indexing the foreign key in the migrations folder next to src
        #[structopt(long = "migration")]
        migration: bool,
        /// Targets SQLite with the sqlx queries and the migration (the default)
        #[structopt(long = "sqlite", conflicts_with_all = &["postgres", "mysql"])]
        sqlite: bool,
        /// Targets PostgreSQL with the sqlx queries and the migration
        #[structopt(long = "postgres", conflicts_with = "mysql")]
        postgres: bool,
        /// Targets MySQL with the sqlx queries and the migration
        #[structopt(long = "mysql")]
        mysql: bool,
        /// The models were generated with --timestamps: the soft deleted rows are skipped
        #[structopt(long = "timestamps")]
        timestamps: bool,
        /// Overwrites the relation file when it already exists, the code written in it is lost
        #[structopt(long = "force")]
        force: bool,
    },
}

const BOOTSTRAP_MAIN: &str = r#"mod helpers;
use actix_web::web;
use helpers::AppState;
use sqlx::SqlitePool;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let pool = SqlitePool::connect("sqlite://data.db?mode=rwc").await.unwrap();
    // sqlx::migrate!().run(&pool).await.unwrap();
    let state = web::Data::new(AppState { pool });

    actix_web::HttpServer::new(move || {
        actix_web::App::new()
            // Actix does not fall through between scopes sharing a prefix,
            // so resources living under the same scope must be registered together
            .service(
                web::scope("v1"), // Where the magic operates
                // Mount your models here with `.configure(<model_name>::configure)`
                // after declaring them with `mod <model_name>;`
            )
            .app_data(state.clone())
    })
    .bind(("127.0.0.1", 8085))?
    .run()
    .await
}
"#;

const BOOTSTRAP_OPENAPI_MAIN: &str = r#"mod helpers;
use actix_web::web;
use apistos::app::{BuildConfig, OpenApiWrapper};
use apistos::info::Info;
use apistos::spec::Spec;
use apistos::SwaggerUIConfig;
use helpers::AppState;
use sqlx::SqlitePool;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let pool = SqlitePool::connect("sqlite://data.db?mode=rwc").await.unwrap();
    // sqlx::migrate!().run(&pool).await.unwrap();
    let state = web::Data::new(AppState { pool });

    actix_web::HttpServer::new(move || {
        let spec = Spec {
            info: Info {
                title: "MyApp REST API".to_string(),
                version: "1.0.0".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        actix_web::App::new()
            .document(spec)
            // Actix does not fall through between scopes sharing a prefix,
            // so resources living under the same scope must be registered together
            .service(
                apistos::web::scope("v1"), // Where the magic operates
                // Mount your models here with `.configure(<model_name>::configure)`
                // after declaring them with `mod <model_name>;`
            )
            .app_data(state.clone())
            .build_with(
                "/openapi.json",
                BuildConfig::default().with(SwaggerUIConfig::new(&"/swagger")),
            )
    })
    .bind(("127.0.0.1", 8085))?
    .run()
    .await
}
"#;

const BOOTSTRAP_HELPERS: &str = r#"use sqlx::SqlitePool;

pub struct AppState {
    pub pool: SqlitePool,
}
"#;

const OPENAPI_IMPORTS: &str = r#"
    use apistos::ApiComponent;
    use schemars::JsonSchema;
    use octopux::gen_documented_endpoint;"#;

const OPENAPI_CONFIGURE: &str = r#"

    // Registers the documented routes of the {entity_lower_case} endpoint
    // (octopux `openapi` feature), to mount with `.configure({entity_lower_case}::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!({entity}, New{entity}, Updatable{entity})(cfg)
    }
"#;

const ENDPOINT_IMPORTS: &str = r#"
    use octopux::gen_endpoint;"#;

const ENDPOINT_CONFIGURE: &str = r#"

    // Registers the routes of the {entity_lower_case} endpoint, to mount with `.configure({entity_lower_case}::configure)`
    pub fn configure(cfg: &mut actix_web::web::ServiceConfig) {
        gen_endpoint!({entity}, New{entity}, Updatable{entity})(cfg)
    }
"#;

const OPENAPI_DERIVES: &str = ", JsonSchema, ApiComponent";

// Columns added by --timestamps, all nullable, `updated_at` is also part of the updatable struct
const TIMESTAMP_COLUMNS: [&str; 3] = ["created_at", "updated_at", "deleted_at"];
const TIMESTAMP_TYPE: &str = "Option<DateTime<Utc>>";
// Type behind the `Id` alias of the generated model
const ID_TYPE: &str = "i64";

#[derive(Debug, PartialEq)]
struct Field {
    name: String,
    ty: String,
    // foreign key of the column, added to the migration
    references: Option<Reference>,
    // unique constraint of the column, added to the migration
    unique: bool,
    // length of a VARCHAR column, the one of the dialect when None
    length: Option<u32>,
}

// Column referenced by a foreign key
#[derive(Debug, Clone, PartialEq)]
struct Reference {
    table: String,
    column: String,
}

// Table created by a migration, proposed as the target of the foreign keys
#[derive(Debug, Clone, PartialEq)]
struct Table {
    name: String,
    columns: Vec<Column>,
}

#[derive(Debug, Clone, PartialEq)]
struct Column {
    name: String,
    sql_type: String,
    // primary key or unique, which the databases require for a referenced column
    unique: bool,
}

// Types proposed when prompting for a field type, the first one is the default
const FIELD_TYPES: &[&str] = &[
    "String", "i32", "i64", "f64", "bool", "Option<String>", "DateTime<Utc>", "NaiveDateTime", "NaiveDate", "NaiveTime", "Vec<u8>",
];

fn field_types_menu() -> String {
    FIELD_TYPES
        .iter()
        .enumerate()
        .map(|(i, ty)| format!("{} {}", magenta(&format!("{})", i + 1)), ty))
        .collect::<Vec<_>>()
        .join("  ")
}

// Resolves a type answer: empty for the default, a number from the menu, or any custom type,
// a trailing `?` makes it optional (`3?` gives `Option<i64>`, `?` gives `Option<String>`)
fn parse_field_type(answer: &str) -> Option<String> {
    if let Some(inner) = answer.strip_suffix('?') {
        return parse_field_type(inner.trim()).map(|ty| if ty.starts_with("Option<") { ty } else { format!("Option<{}>", ty) });
    }
    if answer.is_empty() {
        return Some(FIELD_TYPES[0].to_string());
    }
    match answer.parse::<usize>() {
        Ok(n) => FIELD_TYPES.get(n.wrapping_sub(1)).map(|ty| ty.to_string()),
        Err(_) => Some(answer.to_string()),
    }
}

// Field names are snake_case: they are also the column names, which PostgreSQL folds to lowercase
// when unquoted, so a `OptStr` field would not find its `optstr` column when decoding the rows
fn is_field_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() || c == '_' => chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
        _ => false,
    }
}

// Converts a typed field name to snake_case: `optStr`, `OptStr`, `opt-str` and `opt str` all give `opt_str`,
// acronyms stay together (`HTTPCode` gives `http_code`)
fn to_snake_case(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut snake = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c == '-' || c.is_whitespace() {
            if !snake.is_empty() && !snake.ends_with('_') {
                snake.push('_');
            }
        } else if c.is_uppercase() {
            let prev = if i > 0 { Some(chars[i - 1]) } else { None };
            let next = chars.get(i + 1);
            let boundary = prev.map_or(false, |p| p.is_lowercase() || p.is_ascii_digit())
                || (prev.map_or(false, |p| p.is_uppercase()) && next.map_or(false, |n| n.is_lowercase()));
            if boundary && !snake.ends_with('_') {
                snake.push('_');
            }
            snake.extend(c.to_lowercase());
        } else {
            snake.push(c);
        }
    }
    snake
}

// Asks `message` after a blank line, which spaces the prompts out
fn prompt<R: BufRead, W: Write>(input: &mut R, output: &mut W, message: &str) -> Result<Option<String>, Error> {
    write!(output, "\n{}", message)?;
    output.flush()?;
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    Ok(Some(line.trim().to_string()))
}

// Asks for field names and types until an empty name (or end of input) is entered,
// `timestamps` reserves the `created_at`, `updated_at` and `deleted_at` names,
// with a `dialect`, only accepts types with a column type in its database (see `Dialect::sql_types`),
// with `tables` (the model table and the tables of the migrations), asks for the column each field references,
// with `unique`, asks whether the column of each field is unique
fn read_fields<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    timestamps: bool,
    dialect: Option<Dialect>,
    tables: Option<(&str, &[Table])>,
    unique: bool,
) -> Result<Vec<Field>, Error> {
    let mut fields: Vec<Field> = Vec::new();
    let reserved: &[&str] = if timestamps { &["id", "created_at", "updated_at", "deleted_at"] } else { &["id"] };
    writeln!(output, "{}", bold("Model fields"))?;
    if timestamps {
        writeln!(output, "{}", highlight(&format!("Enter the model fields (empty name to finish), `id: Id` ({}), `created_at`, `updated_at` and `deleted_at` are already declared", ID_TYPE)))?;
    } else {
        writeln!(output, "{}", highlight(&format!("Enter the model fields (empty name to finish), `id: Id` ({}) is already declared", ID_TYPE)))?;
    }
    writeln!(output, "{}", highlight("Wrap a type in `Option<T>` (e.g. `Option<i32>`) to make the field optional, its column is then nullable"))?;
    writeln!(output, "{}", dim(&highlight("Tips: `name:type` skips the type question (`stars:i32`, `stars:2`), a trailing `?` makes the type optional (`2?`), `-` removes the last field")))?;
    loop {
        let message = format!("{} {} ", cyan("?"), bold(&format!("Field {} name ›", fields.len() + 1)));
        let answer = match prompt(input, output, &message)? {
            Some(answer) if !answer.is_empty() => answer,
            _ => break,
        };
        if answer == "-" {
            match fields.pop() {
                Some(field) => writeln!(output, "{}", warning(&format!("Field `{}` removed", field.name)))?,
                None => writeln!(output, "{}", warning("No field to remove"))?,
            }
            continue;
        }
        // `name:type` gives the type with the name
        let (name, inline_type) = match answer.split_once(':') {
            Some((name, ty)) => (name.trim().to_string(), Some(ty.trim().to_string())),
            None => (answer, None),
        };
        let snake = to_snake_case(&name);
        if !is_field_name(&snake) {
            writeln!(output, "{}", failure(&format!("`{}` is not a valid field name, use snake_case", name)))?;
            continue;
        }
        if snake != name {
            writeln!(output, "  {}", dim(&highlight(&format!("`{}` renamed to `{}`", name, snake))))?;
        }
        let name = snake;
        if reserved.contains(&name.as_str()) || fields.iter().any(|f| f.name == name) {
            writeln!(output, "{}", failure(&format!("Field `{}` is already declared", name)))?;
            continue;
        }
        let mut answer = inline_type;
        if answer.is_none() {
            writeln!(output, "  {}", field_types_menu())?;
        }
        let ty = loop {
            let answer = match answer.take() {
                Some(answer) => answer,
                None => {
                    let message = format!(
                        "{} {} {} ",
                        cyan("?"),
                        bold(&format!("Type of {} ›", cyan(&format!("`{}`", name)))),
                        dim(&format!("(number or custom type) [{}]", FIELD_TYPES[0]))
                    );
                    prompt(input, output, &message)?.unwrap_or_default()
                }
            };
            match check_field_type(&answer, dialect) {
                Ok(ty) => break ty,
                Err(error) => {
                    writeln!(output, "{}", failure(&error))?;
                    writeln!(output, "  {}", field_types_menu())?;
                }
            }
        };
        let length = match dialect.and_then(|d| d.sql_column_type(&ty)).and_then(|(sql, _)| varchar_length(sql)) {
            // None for the length of the dialect
            Some(default) => Some(read_length(input, output, &name, dialect.unwrap_or(Dialect::Sqlite), default)?).filter(|l| *l != default),
            None => None,
        };
        let column = dialect.and_then(|d| d.column_type(&ty, length));
        let references = match tables {
            Some((own, existing)) => {
                // the model can reference itself (a `parent_id`), with the fields declared so far
                let mut candidates: Vec<Table> = existing.iter().filter(|t| t.name != own).cloned().collect();
                candidates.push(model_table(own, &fields, dialect));
                read_reference(input, output, &name, column.as_deref(), dialect, own, &candidates)?
            }
            None => None,
        };
        let unique = unique && read_unique(input, output, &name, column.as_deref(), dialect)?;
        let field = Field { name, ty, references, unique, length };
        writeln!(output, "  {} {}", green("✔"), field_line(&field, 0))?;
        fields.push(field);
    }
    if !fields.is_empty() {
        writeln!(output, "{}", fields_summary(&fields, timestamps))?;
    }
    Ok(fields)
}

// The length of a `VARCHAR(n)` column type
fn varchar_length(sql: &str) -> Option<u32> {
    sql.strip_prefix("VARCHAR(")?.strip_suffix(')')?.parse().ok()
}

// Asks for the length of the VARCHAR column of the field `name`, `default` when empty
fn read_length<R: BufRead, W: Write>(input: &mut R, output: &mut W, name: &str, dialect: Dialect, default: u32) -> Result<u32, Error> {
    let max = dialect.max_varchar_length();
    loop {
        let message = format!(
            "{} {} {} ",
            cyan("?"),
            bold(&format!("Length of {} ›", cyan(&format!("`{}`", name)))),
            dim(&format!("(VARCHAR, 1 to {}) [{}]", max, default))
        );
        let answer = match prompt(input, output, &message)? {
            Some(answer) if !answer.is_empty() => answer,
            _ => return Ok(default),
        };
        match answer.parse::<u32>() {
            Ok(length) if (1..=max).contains(&length) => return Ok(length),
            _ => writeln!(output, "{}", failure(&format!("`{}` is not a {} VARCHAR length, pick 1 to {}", answer, dialect.name(), max)))?,
        }
    }
}

// Asks whether the `column` of the field `name` is unique, no by default,
// warns when MySQL refuses a unique index on its column type
fn read_unique<R: BufRead, W: Write>(input: &mut R, output: &mut W, name: &str, column: Option<&str>, dialect: Option<Dialect>) -> Result<bool, Error> {
    let message = format!("{} {} {} ", cyan("?"), bold(&format!("Is {} unique ›", cyan(&format!("`{}`", name)))), dim("(y/N)"));
    let answer = prompt(input, output, &message)?.unwrap_or_default().to_lowercase();
    let unique = matches!(answer.as_str(), "y" | "yes");
    if let (true, Some(Dialect::Mysql), Some(sql)) = (unique, dialect, column) {
        if sql.ends_with("BLOB") || sql.ends_with("TEXT") {
            writeln!(output, "{}", warning(&format!("MySQL refuses a unique index on the {} column `{}` without a key length, `-` removes the field", sql, name)))?;
        }
    }
    Ok(unique)
}

// The type of a type answer (see `parse_field_type`), or why it is refused,
// with a `dialect`, only accepts types with a column type in its database
fn check_field_type(answer: &str, dialect: Option<Dialect>) -> Result<String, String> {
    match (parse_field_type(answer), dialect) {
        (Some(ty), Some(dialect)) if dialect.sql_column_type(&ty).is_none() => Err(format!(
            "`{}` has no {} column type, use one of {}, or Option<T> of them",
            ty,
            dialect.name(),
            dialect.sql_types().iter().map(|(ty, _)| *ty).collect::<Vec<_>>().join(", ")
        )),
        (Some(ty), _) => Ok(ty),
        (None, _) => Err(format!("`{}` is not in the list, pick 1 to {}", answer, FIELD_TYPES.len())),
    }
}

// `name: Type`, the name padded to `width`, optional types flagged as nullable and unique columns as unique, followed by the referenced column
fn field_line(field: &Field, width: usize) -> String {
    let nullable = if field.ty.starts_with("Option<") { dim(" (nullable)") } else { String::new() };
    let unique = if field.unique { dim(" (unique)") } else { String::new() };
    let length = field.length.map_or(String::new(), |l| dim(&format!(" (length {})", l)));
    let references = match &field.references {
        Some(r) => format!(" {} {}", dim("→"), magenta(&format!("{} ({})", r.table, r.column))),
        None => String::new(),
    };
    format!("{}: {}{}{}{}{}", bold(&format!("{:<width$}", field.name, width = width)), yellow(&field.ty), length, nullable, unique, references)
}

// The table of the model being generated, with its `id` and the fields declared so far
fn model_table(name: &str, fields: &[Field], dialect: Option<Dialect>) -> Table {
    let sql_type = |f: &Field| dialect.and_then(|d| d.column_type(&f.ty, f.length)).unwrap_or_default();
    let id_type = dialect.map_or(String::new(), |d| d.id_column().split_whitespace().nth(1).unwrap_or_default().to_string());
    let id = Column { name: "id".to_string(), sql_type: id_type, unique: true };
    let columns = fields.iter().map(|f| Column { name: f.name.clone(), sql_type: sql_type(f), unique: f.unique });
    Table { name: name.to_string(), columns: std::iter::once(id).chain(columns).collect() }
}

// The item of `items` picked by its number in the menu or by its name
fn pick<'a, T>(items: &'a [T], answer: &str, name: impl Fn(&T) -> &str) -> Option<&'a T> {
    match answer.parse::<usize>() {
        Ok(n) => items.get(n.wrapping_sub(1)),
        Err(_) => items.iter().find(|item| name(item).eq_ignore_ascii_case(answer)),
    }
}

fn menu<T>(items: &[T], label: impl Fn(&T) -> String) -> String {
    items
        .iter()
        .enumerate()
        .map(|(i, item)| format!("{} {}", magenta(&format!("{})", i + 1)), label(item)))
        .collect::<Vec<_>>()
        .join("  ")
}

// Asks for the table and the column referenced by the field `name` of column type `sql`, None when it references nothing,
// warns when the column is not unique or not of the type of the field, which the databases refuse
fn read_reference<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    name: &str,
    sql: Option<&str>,
    dialect: Option<Dialect>,
    own: &str,
    tables: &[Table],
) -> Result<Option<Reference>, Error> {
    let tables_menu = menu(tables, |t| if t.name == own { format!("{} {}", t.name, dim("(this model)")) } else { t.name.clone() });
    writeln!(output, "  {}", tables_menu)?;
    let table = loop {
        let message = format!(
            "{} {} {} ",
            cyan("?"),
            bold(&format!("Table referenced by {} ›", cyan(&format!("`{}`", name)))),
            dim("(number or name, empty for none)")
        );
        let answer = prompt(input, output, &message)?.unwrap_or_default();
        if answer.is_empty() {
            return Ok(None);
        }
        match pick(tables, &answer, |t| &t.name) {
            Some(table) => break table,
            None => {
                writeln!(output, "{}", failure(&format!("`{}` is not a known table, pick 1 to {}", answer, tables.len())))?;
                writeln!(output, "  {}", tables_menu)?;
            }
        }
    };
    // the primary key by default
    let Some(default) = table.columns.iter().find(|c| c.unique).or(table.columns.first()) else {
        writeln!(output, "{}", warning(&format!("Table `{}` has no column, `{}` references nothing", table.name, name)))?;
        return Ok(None);
    };
    let columns_menu = menu(&table.columns, |c| {
        let key = if c.unique { dim(" (unique)") } else { String::new() };
        format!("{} {}{}", c.name, dim(&c.sql_type), key)
    });
    writeln!(output, "  {}", columns_menu)?;
    let column = loop {
        let message = format!(
            "{} {} {} ",
            cyan("?"),
            bold(&format!("Column of {} referenced by {} ›", cyan(&format!("`{}`", table.name)), cyan(&format!("`{}`", name)))),
            dim(&format!("(number or name) [{}]", default.name))
        );
        let answer = prompt(input, output, &message)?.unwrap_or_default();
        if answer.is_empty() {
            break default;
        }
        match pick(&table.columns, &answer, |c| &c.name) {
            Some(column) => break column,
            None => {
                writeln!(output, "{}", failure(&format!("`{}` is not a column of `{}`, pick 1 to {}", answer, table.name, table.columns.len())))?;
                writeln!(output, "  {}", columns_menu)?;
            }
        }
    };
    let target = format!("{}.{}", table.name, column.name);
    if !column.unique {
        writeln!(output, "{}", warning(&format!("`{}` is neither a primary key nor unique, the database refuses the foreign key without a unique index on it", target)))?;
    }
    if let (Some(dialect), Some(sql)) = (dialect, sql) {
        if !dialect.same_column_type(sql, &column.sql_type) {
            writeln!(
                output,
                "{}",
                warning(&format!("`{}` is {} and `{}` is {}, the foreign key may be refused, `-` removes the field", name, sql, target, column.sql_type))
            )?;
        }
    }
    Ok(Some(Reference { table: table.name.clone(), column: column.name.clone() }))
}

// Removes the comments, the quotes and the schema of an SQL name: `"public"."author"` gives `author`
fn sql_name(name: &str) -> String {
    let name = name.rsplit('.').next().unwrap_or(name);
    name.trim_matches(|c| matches!(c, '"' | '`' | '[' | ']')).to_string()
}

// Splits on the commas outside parentheses, `DECIMAL(10, 2)` stays whole
fn split_top_level(body: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut depth, mut start) = (0, 0);
    for (i, c) in body.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(body[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(body[start..].trim());
    parts.into_iter().filter(|p| !p.is_empty()).collect()
}

// Keywords ending the type of a column definition
const COLUMN_CONSTRAINTS: &[&str] = &[
    "NOT", "NULL", "PRIMARY", "REFERENCES", "DEFAULT", "UNIQUE", "AUTO_INCREMENT", "AUTOINCREMENT", "CHECK", "CONSTRAINT", "GENERATED", "COLLATE",
];

// Columns of a CREATE TABLE body, the single column PRIMARY KEY and UNIQUE table constraints mark their column unique
fn parse_columns(body: &str) -> Vec<Column> {
    let mut columns = Vec::new();
    let mut unique_keys = Vec::new();
    for definition in split_top_level(body) {
        let upper = definition.to_ascii_uppercase();
        let words: Vec<&str> = definition.split_whitespace().collect();
        let first = upper.split_whitespace().next().unwrap_or_default();
        if ["CONSTRAINT", "PRIMARY", "UNIQUE", "FOREIGN", "CHECK", "KEY", "INDEX"].contains(&first) {
            let unique = (upper.contains("PRIMARY KEY") || upper.contains("UNIQUE")) && !upper.contains("FOREIGN KEY");
            if let (true, Some(open), Some(close)) = (unique, definition.find('('), definition.find(')')) {
                let keys: Vec<&str> = definition[open + 1..close].split(',').collect();
                if let [key] = keys.as_slice() {
                    unique_keys.push(sql_name(key.trim()));
                }
            }
            continue;
        }
        let sql_type: Vec<&str> = words[1..]
            .iter()
            .take_while(|w| !COLUMN_CONSTRAINTS.contains(&w.to_ascii_uppercase().as_str()))
            .copied()
            .collect();
        let unique = upper.contains("PRIMARY KEY") || upper.split_whitespace().any(|w| w == "UNIQUE");
        columns.push(Column { name: sql_name(words[0]), sql_type: sql_type.join(" "), unique });
    }
    for column in columns.iter_mut() {
        column.unique |= unique_keys.contains(&column.name);
    }
    columns
}

// Tables created by the CREATE TABLE statements of a migration
fn parse_tables(sql: &str) -> Vec<Table> {
    let sql = sql.lines().map(|line| line.split("--").next().unwrap_or_default()).collect::<Vec<_>>().join("\n");
    // same byte offsets as `sql`
    let upper = sql.to_ascii_uppercase();
    let mut tables = Vec::new();
    let mut rest = 0;
    while let Some(start) = upper[rest..].find("CREATE TABLE") {
        let mut pos = rest + start + "CREATE TABLE".len();
        pos += upper[pos..].len() - upper[pos..].trim_start().len();
        if upper[pos..].starts_with("IF NOT EXISTS") {
            pos += "IF NOT EXISTS".len();
        }
        let Some(open) = sql[pos..].find('(').map(|i| pos + i) else { break };
        let mut depth = 0;
        let close = sql[open..].char_indices().find_map(|(i, c)| {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            (depth == 0).then_some(open + i)
        });
        let Some(close) = close else { break };
        tables.push(Table { name: sql_name(sql[pos..open].trim()), columns: parse_columns(&sql[open + 1..close]) });
        rest = close;
    }
    tables
}

// Tables created by the migrations of `dir`, in the order of the migrations,
// a table created by several migrations keeps its first definition, which `IF NOT EXISTS` applies
fn migration_tables(dir: &Path) -> Vec<Table> {
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().map_or(false, |e| e == "sql") && !path.to_string_lossy().ends_with(".down.sql"))
        .collect();
    paths.sort();
    let mut tables: Vec<Table> = Vec::new();
    for path in paths {
        let Ok(sql) = fs::read_to_string(&path) else { continue };
        for table in parse_tables(&sql) {
            if !tables.iter().any(|t| t.name == table.name) {
                tables.push(table);
            }
        }
    }
    tables
}

// Database of a connection url, from its scheme
fn url_dialect(url: &str) -> Option<Dialect> {
    match url.split(':').next()? {
        "sqlite" => Some(Dialect::Sqlite),
        "postgres" | "postgresql" => Some(Dialect::Postgres),
        "mysql" | "mariadb" => Some(Dialect::Mysql),
        _ => None,
    }
}

// Table of the sqlx migrations, not a model to reference
const SQLX_MIGRATIONS_TABLE: &str = "_sqlx_migrations";

// Columns of the current schema, with their udt name (`int8`, `_text` for an array of text)
// and whether a single column primary key or unique constraint covers them
const POSTGRES_COLUMNS: &str = "SELECT c.table_name::text, c.column_name::text, c.udt_name::text,
    EXISTS (
        SELECT 1 FROM information_schema.table_constraints tc
        JOIN information_schema.key_column_usage k
            ON k.constraint_schema = tc.constraint_schema AND k.constraint_name = tc.constraint_name
        WHERE tc.table_schema = c.table_schema AND tc.table_name = c.table_name AND k.column_name = c.column_name
            AND tc.constraint_type IN ('PRIMARY KEY', 'UNIQUE')
            AND (SELECT count(*) FROM information_schema.key_column_usage k2
                 WHERE k2.constraint_schema = tc.constraint_schema AND k2.constraint_name = tc.constraint_name) = 1
    )
FROM information_schema.columns c
JOIN information_schema.tables t ON t.table_schema = c.table_schema AND t.table_name = c.table_name
WHERE c.table_schema = current_schema() AND t.table_type = 'BASE TABLE' AND c.table_name <> '_sqlx_migrations'
ORDER BY c.table_name, c.ordinal_position";

// information_schema columns are cast, MySQL 8 returns some of them as binary strings
const MYSQL_COLUMNS: &str = "SELECT CAST(c.TABLE_NAME AS CHAR), CAST(c.COLUMN_NAME AS CHAR), CAST(c.COLUMN_TYPE AS CHAR),
    CAST(c.COLUMN_KEY IN ('PRI', 'UNI') AS SIGNED)
FROM information_schema.COLUMNS c
JOIN information_schema.TABLES t ON t.TABLE_SCHEMA = c.TABLE_SCHEMA AND t.TABLE_NAME = c.TABLE_NAME
WHERE c.TABLE_SCHEMA = DATABASE() AND t.TABLE_TYPE = 'BASE TABLE' AND c.TABLE_NAME <> '_sqlx_migrations'
ORDER BY c.TABLE_NAME, c.ORDINAL_POSITION";

// Groups the (table, column) rows, ordered by table, into tables
fn group_columns(rows: impl IntoIterator<Item = (String, Column)>) -> Vec<Table> {
    let mut tables: Vec<Table> = Vec::new();
    for (table, column) in rows {
        match tables.last_mut() {
            Some(last) if last.name == table => last.columns.push(column),
            _ => tables.push(Table { name: table, columns: vec![column] }),
        }
    }
    tables
}

// `int8` gives `INT8`, the `_text` arrays `TEXT[]`, as sqlx names the column types
fn postgres_column_type(udt: &str) -> String {
    match udt.strip_prefix('_') {
        Some(element) => format!("{}[]", element.to_uppercase()),
        None => udt.to_uppercase(),
    }
}

// Tables of the database of `url`, read without writing anything,
// a relative SQLite file is looked up from `root`, the crate root, when missing from the working directory
async fn database_tables(url: &str, root: &Path) -> Result<Vec<Table>, String> {
    use sqlx::Connection;
    let error = |e: sqlx::Error| e.to_string();
    match url_dialect(url) {
        Some(Dialect::Sqlite) => {
            use std::str::FromStr;
            let mut options = sqlx::sqlite::SqliteConnectOptions::from_str(url).map_err(error)?;
            let file = options.get_filename().to_path_buf();
            if file.is_relative() && !file.exists() && root.join(&file).exists() {
                options = options.filename(root.join(&file));
            }
            let mut conn = sqlx::SqliteConnection::connect_with(&options.read_only(true).create_if_missing(false)).await.map_err(error)?;
            // the CREATE TABLE statements, as for the migrations
            let statements: Vec<Option<String>> = sqlx::query_scalar(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name <> $1 ORDER BY name",
            )
            .bind(SQLX_MIGRATIONS_TABLE)
            .fetch_all(&mut conn)
            .await
            .map_err(error)?;
            Ok(statements.iter().flatten().flat_map(|sql| parse_tables(sql)).collect())
        }
        Some(Dialect::Postgres) => {
            let mut conn = sqlx::PgConnection::connect(url).await.map_err(error)?;
            let rows: Vec<(String, String, String, bool)> = sqlx::query_as(POSTGRES_COLUMNS).fetch_all(&mut conn).await.map_err(error)?;
            Ok(group_columns(rows.into_iter().map(|(table, name, udt, unique)| {
                (table, Column { name, sql_type: postgres_column_type(&udt), unique })
            })))
        }
        Some(Dialect::Mysql) => {
            let mut conn = sqlx::MySqlConnection::connect(url).await.map_err(error)?;
            let rows: Vec<(String, String, String, i64)> = sqlx::query_as(MYSQL_COLUMNS).fetch_all(&mut conn).await.map_err(error)?;
            Ok(group_columns(rows.into_iter().map(|(table, name, ty, key)| {
                (table, Column { name, sql_type: ty.to_uppercase(), unique: key != 0 })
            })))
        }
        None => Err("unsupported scheme, use sqlite:, postgres: or mysql:".to_string()),
    }
}

// Tables proposed to the foreign keys: those of the DATABASE_URL database when it is set and reachable,
// those created by the migrations otherwise
fn known_tables(dialect: Dialect) -> Result<Vec<Table>, Error> {
    let cwd = std::env::current_dir()?;
    let migrations = migrations_dir(&cwd);
    let root = migrations.parent().map_or(PathBuf::new(), Path::to_path_buf);
    if let Ok(url) = std::env::var("DATABASE_URL") {
        if let Some(db) = url_dialect(&url).filter(|db| *db != dialect) {
            println!("{}", warning(&format!("`DATABASE_URL` is a {} database and the migration targets {}", db.name(), dialect.name())));
        }
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        match runtime.block_on(database_tables(&url, &root)) {
            Ok(tables) => {
                println!("{}", success(&format!("{} table{} read from `DATABASE_URL`", tables.len(), if tables.len() > 1 { "s" } else { "" })));
                return Ok(tables);
            }
            Err(e) => println!("{}", warning(&format!("Tables of `DATABASE_URL` not read ({}), the migrations are read instead", e))),
        }
    }
    let tables = migration_tables(&migrations);
    if tables.is_empty() {
        println!("{}", warning(&format!("No table in {}, the fields can only reference the model itself", migrations.display())));
    } else {
        println!("{}", success(&format!("{} table{} read from {}", tables.len(), if tables.len() > 1 { "s" } else { "" }, migrations.display())));
    }
    Ok(tables)
}

// Recap of the declared fields, with the `id` and the timestamps declared by the CLI dimmed
fn fields_summary(fields: &[Field], timestamps: bool) -> String {
    let declared: Vec<(&str, &str)> = std::iter::once(("id", "Id"))
        .chain(timestamps.then_some(TIMESTAMP_COLUMNS.map(|c| (c, TIMESTAMP_TYPE))).into_iter().flatten())
        .collect();
    let width = fields.iter().map(|f| f.name.len()).chain(declared.iter().map(|(n, _)| n.len())).max().unwrap_or(0);
    let mut lines = vec![format!("\n{}", bold(&format!("{} field{} declared", fields.len(), if fields.len() > 1 { "s" } else { "" })))];
    lines.push(format!("  {}", dim(&format!("{:<width$}: Id ({})", "id", ID_TYPE, width = width))));
    lines.extend(fields.iter().map(|f| format!("  {}", field_line(f, width))));
    lines.extend(declared.iter().skip(1).map(|(n, ty)| format!("  {}", dim(&format!("{:<width$}: {}", n, ty, width = width)))));
    lines.join("\n") + "\n"
}

// Bodies of find, list, delete, save and update, as left to the user without --sqlx
const EMPTY_BODIES: [&str; 5] = [
    "            // fetch from somwhere with id",
    "            // list",
    "            // hard or soft delete",
    "            // persist",
    "            // update in db",
];

// Pagination parameters of the list query, read by SqlxModel, only generated with --sqlx
const LIST_QUERY_FIELDS: &str = r#"
        /// Number of rows to skip
        pub offset: Option<usize>,
        /// Maximum number of rows to return (20 by default, 100 at most)
        pub limit: Option<usize>,
    "#;

// Clamps the pagination parameters of the list query before binding them
const LIST_PAGINATION: &str = "            let offset = query.offset.unwrap_or(0) as i64;
            let limit = query.limit.map_or(DEFAULT_LIMIT, |l| (l as i64).min(MAX_LIMIT));";

const RELATION_LIMITS: &str = r#"
    /// Number of children returned when no limit is given
    const DEFAULT_LIMIT: i64 = 20;
    /// Maximum number of children returned
    const MAX_LIMIT: i64 = 100;"#;

// Database targeted by the sqlx queries and the migration
#[derive(Debug, Clone, Copy, PartialEq)]
enum Dialect {
    Sqlite,
    Postgres,
    Mysql,
}

// Column types of the field types, as sqlx declares them (`sqlx::Type::type_info`)
// so the columns decode into the fields, the field types are listed with the path used in the models
macro_rules! sql_types {
    ($db:ty; $($ty:ty),* $(,)?) => {
        vec![$( (stringify!($ty), <$ty as sqlx::Type<$db>>::type_info().name().to_string()) ),*]
    };
}

fn sqlite_types() -> Vec<(&'static str, String)> {
    sql_types!(sqlx::Sqlite;
        String, i8, i16, i32, u8, u16, i64, u32, u64, f32, f64, bool,
        DateTime<Utc>, NaiveDateTime, NaiveDate, NaiveTime, Vec<u8>,
    )
}

// Replaces the column types sqlx declares by the ones of `overrides`
fn with_overrides(mut types: Vec<(&'static str, String)>, overrides: &[(&str, &str)]) -> Vec<(&'static str, String)> {
    for (ty, sql) in types.iter_mut() {
        if let Some((_, over)) = overrides.iter().find(|(t, _)| t == ty) {
            *sql = over.to_string();
        }
    }
    types
}

// Strings are bounded as for MySQL, where sqlx declares an unbounded TEXT
const POSTGRES_OVERRIDES: &[(&str, &str)] = &[("String", "VARCHAR(255)")];

// PostgreSQL has no unsigned integers and sqlx maps i8 to "char", Vec<T> are arrays
fn postgres_types() -> Vec<(&'static str, String)> {
    let types = sql_types!(sqlx::Postgres;
        String, i16, i32, i64, f32, f64, bool,
        DateTime<Utc>, NaiveDateTime, NaiveDate, NaiveTime, Vec<u8>,
        Vec<String>, Vec<i16>, Vec<i32>, Vec<i64>, Vec<f32>, Vec<f64>, Vec<bool>,
        Vec<DateTime<Utc>>, Vec<NaiveDateTime>, Vec<NaiveDate>, Vec<NaiveTime>, Vec<Vec<u8>>,
    );
    with_overrides(types, POSTGRES_OVERRIDES)
}

// sqlx names a VARCHAR without its length, which is not a valid column type.
// sqlx writes DateTime<Utc> as its UTC date and time, stored as is by DATETIME
// where TIMESTAMP (the sqlx type) would convert it from the session time zone,
// and with microseconds as the other databases
const MYSQL_OVERRIDES: &[(&str, &str)] = &[
    ("String", "VARCHAR(255)"),
    ("DateTime<Utc>", "DATETIME(6)"),
    ("NaiveDateTime", "DATETIME(6)"),
    ("NaiveTime", "TIME(6)"),
];

fn mysql_types() -> Vec<(&'static str, String)> {
    let types = sql_types!(sqlx::MySql;
        String, i8, i16, i32, i64, u8, u16, u32, u64, f32, f64, bool,
        DateTime<Utc>, NaiveDateTime, NaiveDate, NaiveTime, Vec<u8>,
    );
    with_overrides(types, MYSQL_OVERRIDES)
}

impl Dialect {
    fn from_flags(postgres: bool, mysql: bool) -> Dialect {
        match (postgres, mysql) {
            (true, _) => Dialect::Postgres,
            (_, true) => Dialect::Mysql,
            _ => Dialect::Sqlite,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Dialect::Sqlite => "SQLite",
            Dialect::Postgres => "PostgreSQL",
            Dialect::Mysql => "MySQL",
        }
    }

    fn sql_types(self) -> &'static [(&'static str, String)] {
        static SQLITE: OnceLock<Vec<(&str, String)>> = OnceLock::new();
        static POSTGRES: OnceLock<Vec<(&str, String)>> = OnceLock::new();
        static MYSQL: OnceLock<Vec<(&str, String)>> = OnceLock::new();
        match self {
            Dialect::Sqlite => SQLITE.get_or_init(sqlite_types),
            Dialect::Postgres => POSTGRES.get_or_init(postgres_types),
            Dialect::Mysql => MYSQL.get_or_init(mysql_types),
        }
    }

    // Column of the `id: i64` primary key
    fn id_column(self) -> &'static str {
        match self {
            Dialect::Sqlite => "id INTEGER PRIMARY KEY AUTOINCREMENT",
            Dialect::Postgres => "id BIGSERIAL PRIMARY KEY",
            Dialect::Mysql => "id BIGINT AUTO_INCREMENT PRIMARY KEY",
        }
    }

    fn sql_type(self, ty: &str) -> Option<&'static str> {
        // chrono types can be written with their path
        let ty = ty.trim_start_matches("chrono::");
        self.sql_types().iter().find(|(t, _)| *t == ty).map(|(_, sql)| sql.as_str())
    }

    // Column type of a field type and whether it is NOT NULL, `Option<T>` fields are nullable,
    // None when the type has no column type
    fn sql_column_type(self, ty: &str) -> Option<(&'static str, bool)> {
        let ty: String = ty.chars().filter(|c| !c.is_whitespace()).collect();
        match ty.strip_prefix("Option<").and_then(|t| t.strip_suffix('>')) {
            Some(inner) => self.sql_type(inner).map(|sql| (sql, false)),
            None => self.sql_type(&ty).map(|sql| (sql, true)),
        }
    }

    // The column type of a field type, `length` replacing the one of a VARCHAR
    fn column_type(self, ty: &str, length: Option<u32>) -> Option<String> {
        self.sql_column_type(ty).map(|(sql, _)| self.with_length(sql, length))
    }

    fn with_length(self, sql: &str, length: Option<u32>) -> String {
        match (varchar_length(sql), length) {
            (Some(_), Some(length)) => format!("VARCHAR({})", length),
            _ => sql.to_string(),
        }
    }

    // PostgreSQL limits a VARCHAR to 10485760 characters, MySQL to 65535 bytes in a row,
    // 16383 characters of utf8mb4
    fn max_varchar_length(self) -> u32 {
        match self {
            Dialect::Mysql => 16383,
            _ => 10_485_760,
        }
    }

    // `$1, $2...` for SQLite and PostgreSQL, `?` for MySQL
    // Whether a column of type `a` can reference a column of type `b`, after the aliases of the database,
    // SQLite does not check the types of the foreign keys
    fn same_column_type(self, a: &str, b: &str) -> bool {
        let canonical = |sql: &str| {
            let sql = sql.split_whitespace().collect::<Vec<_>>().join(" ").to_ascii_uppercase();
            let alias = match (self, sql.as_str()) {
                (Dialect::Postgres, "BIGSERIAL" | "SERIAL8" | "BIGINT") => "INT8",
                (Dialect::Postgres, "SERIAL" | "SERIAL4" | "INTEGER" | "INT") => "INT4",
                (Dialect::Postgres, "SMALLSERIAL" | "SERIAL2" | "SMALLINT") => "INT2",
                (Dialect::Postgres, "DOUBLE PRECISION") => "FLOAT8",
                (Dialect::Postgres, "REAL") => "FLOAT4",
                (Dialect::Postgres, "BOOLEAN") => "BOOL",
                (Dialect::Postgres, "TIMESTAMP WITH TIME ZONE") => "TIMESTAMPTZ",
                // a VARCHAR, of any length, can reference a TEXT
                (Dialect::Postgres, s) if s == "TEXT" || s.starts_with("VARCHAR") || s.starts_with("CHARACTER VARYING") => "TEXT",
                (Dialect::Mysql, "INTEGER") => "INT",
                (Dialect::Mysql, "BOOL" | "TINYINT(1)") => "BOOLEAN",
                _ => return sql,
            };
            alias.to_string()
        };
        self == Dialect::Sqlite || canonical(a) == canonical(b)
    }

    fn placeholders(self, count: usize) -> Vec<String> {
        match self {
            Dialect::Mysql => vec!["?".to_string(); count],
            _ => (1..=count).map(|i| format!("${}", i)).collect(),
        }
    }
}

// A query_as statement of `result` rows bound to `var`
fn sqlx_fetch_all(var: &str, result: &str, sql: &str, binds: &[String]) -> String {
    let binds: String = binds
        .iter()
        .map(|b| format!("\n            .bind({})", b))
        .collect();
    format!(
        "            let {} = sqlx::query_as::<_, {}>(\n                \"{}\",\n            ){}\n            .fetch_all(&state.pool)\n            .await?;",
        var, result, sql, binds
    )
}

// Fails with the fields whose type has no column type in the `dialect` database
fn render_migration(name: &str, fields: &[Field], timestamps: bool, dialect: Dialect) -> Result<String, String> {
    let mut columns = vec![dialect.id_column().to_string()];
    let mut unmapped = Vec::new();
    for field in fields {
        match dialect.sql_column_type(&field.ty).map(|(sql, not_null)| (dialect.with_length(sql, field.length), not_null)) {
            Some((sql, true)) => columns.push(format!("{} {} NOT NULL", field.name, sql)),
            Some((sql, false)) => columns.push(format!("{} {}", field.name, sql)),
            None => unmapped.push(format!("{}: {}", field.name, field.ty)),
        }
    }
    if !unmapped.is_empty() {
        return Err(format!("no {} column type for {}", dialect.name(), unmapped.join(", ")));
    }
    if timestamps {
        // nullable, as their Option<DateTime<Utc>> fields
        let sql = dialect.sql_type("DateTime<Utc>").unwrap_or_default();
        columns.extend(TIMESTAMP_COLUMNS.map(|c| format!("{} {}", c, sql)));
    }
    // table constraints, the inline REFERENCES are ignored by MySQL
    columns.extend(fields.iter().filter(|f| f.unique).map(|f| format!("UNIQUE ({})", f.name)));
    columns.extend(fields.iter().filter_map(|f| {
        f.references.as_ref().map(|r| format!("FOREIGN KEY ({}) REFERENCES {} ({})", f.name, r.table, r.column))
    }));
    Ok(format!(
        "CREATE TABLE IF NOT EXISTS {} (\n    {}\n);\n",
        name.to_lowercase(),
        columns.join(",\n    ")
    ))
}

// Octopus drawn at the top of every generated file
const LOGO: &str = r#"
                        -----
                    -------------
                  -----  ----------
                 ---  --------------
                ---  ----------------
                --- -----------------
                --- -----------------
                --- -----------------
                ---------------------
      -----      -------------------       -----
     -------      --  ---------- --      -------
         ----      ---------------      -----
          ---      ---------------      ----
         ----     -----------------     ----
       ------   ----------------------   ------
   --------  ---------------------------  --------
  ------   -------------------------- ----   -------
 ----    -----  ---------- --- ------- -----    -----
----  ------  -------- --- --- ---- ---  ------  ----
----        ---- ----  --- ---- ---- -----       ----
 ----   ------  ----  ---- ----  ----   ------  -----
 ------      ------   ---- -----  ------      ------
   ---------------    ----  ----    --------------
     ----------       ----  ----      ----------
                      ----  ----
                ---   ---- -----   --
              ------  ---- ----- -------
             -------  ---- ----- --------
             ----    ----   -----    ----
             -----------     -----------
              ---------        --------
"#;

// LOGO and the do-not-edit notice, commented with `comment` (`//` for Rust, `--` for SQL)
fn generated_header(comment: &str) -> String {
    let notice = ["Don't touch this file.", "It is automatically generated by the octopux crate cli."];
    LOGO.trim_matches('\n')
        .lines()
        .chain([""])
        .chain(notice)
        .map(|line| format!("{} {}", comment, line).trim_end().to_string() + "\n")
        .collect::<String>()
        + "\n"
}

// `content` of a generated file, preceded by its header
fn with_header(comment: &str, content: &str) -> String {
    generated_header(comment) + content.trim_start_matches('\n')
}

// Exits when `path` exists and is not to be overwritten, before prompting for anything
fn refuse_overwrite(path: &str, force: bool, what: &str) {
    if !force && Path::new(path).exists() {
        eprintln!("{}", failure(&format!("{} already exists, use --force to overwrite it, {} not generated", path, what)));
        process::exit(1);
    }
}

// `file` in the src folder of the working directory when it exists, in the working directory otherwise
fn source_path(cwd: &Path, file: &str) -> String {
    if cwd.join("src").is_dir() { format!("src/{}", file) } else { file.to_string() }
}

// `<timestamp>_<suffix>.sql` in the migrations folder next to src
fn migration_path(suffix: &str) -> Result<PathBuf, Error> {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    Ok(migrations_dir(&std::env::current_dir()?).join(format!("{}_{}.sql", migration_timestamp(secs), suffix)))
}

// Writes the migration at `path` (see `migration_path`), creating the migrations folder
fn write_migration(path: &Path, sql: &str) -> Result<(), Error> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, with_header("--", sql))?;
    println!("{}", success(&format!("Successfully generated migration {}", path.display())));
    Ok(())
}

// Recap of the files about to be written, each with whether it overwrites an existing file
fn changes_summary(files: &[(String, bool)]) -> String {
    let mut lines = vec![bold(&format!("{} file{} to write", files.len(), if files.len() > 1 { "s" } else { "" }))];
    lines.extend(files.iter().map(|(path, overwrites)| {
        if *overwrites {
            format!("  {} {} {}", yellow("~"), path, yellow("(overwritten)"))
        } else {
            format!("  {} {}", green("+"), path)
        }
    }));
    lines.join("\n")
}

// Shows the recap of the files and asks for saving them, only an explicit no refuses,
// so that a session is not lost on an empty answer, and the fields can still be piped
fn confirm_save<R: BufRead, W: Write>(input: &mut R, output: &mut W, files: &[(String, bool)]) -> Result<bool, Error> {
    writeln!(output, "{}", changes_summary(files))?;
    let answer = prompt(input, output, &format!("{} {} {} ", cyan("?"), bold("Save the changes?"), dim("(Y/n)")))?;
    Ok(!matches!(answer.as_deref().map(str::to_lowercase).as_deref(), Some("n" | "no")))
}

// UTC timestamp prefixing the sqlx migrations, as YYYYMMDDHHMMSS
fn migration_timestamp(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    // civil date from days since 1970-01-01 (Howard Hinnant's algorithm)
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    format!(
        "{:04}{:02}{:02}{:02}{:02}{:02}",
        year, month, day, rem / 3600, rem % 3600 / 60, rem % 60
    )
}

// migrations folder of the crate, next to its src folder, relative to the working directory
fn migrations_dir(cwd: &Path) -> PathBuf {
    let mut dir = PathBuf::new();
    for ancestor in cwd.ancestors() {
        dir.push("..");
        if ancestor.file_name().map_or(false, |n| n == "src") {
            return dir.join("migrations");
        }
    }
    // outside of src, the working directory is taken as the crate root
    PathBuf::from("migrations")
}

// chrono import needed by the timestamps and the date fields, empty when none is used
fn chrono_imports(fields: &[Field], timestamps: bool) -> String {
    // whole type names, `NaiveDateTime` must not import `DateTime`
    let uses = |name: &str| {
        fields
            .iter()
            .any(|f| f.ty.split(|c: char| !c.is_ascii_alphanumeric() && c != '_').any(|t| t == name))
    };
    let mut names = Vec::new();
    for name in ["DateTime", "NaiveDate", "NaiveDateTime", "NaiveTime", "Utc"] {
        if uses(name) || (timestamps && (name == "DateTime" || name == "Utc")) {
            names.push(name);
        }
    }
    if names.is_empty() {
        String::new()
    } else {
        format!("\n    use chrono::{{{}}};", names.join(", "))
    }
}

fn struct_fields(fields: &[Field]) -> String {
    fields
        .iter()
        .map(|f| format!("\n        pub {}: {},", f.name, f.ty))
        .collect()
}

// `#[sqlx_model(...)]` of the model structs, `model` names the model returned by `save`
fn sqlx_model_attribute(dialect: Dialect, model: Option<&str>, timestamps: bool, soft_delete: bool) -> String {
    let database = match dialect {
        Dialect::Sqlite => "sqlite",
        Dialect::Postgres => "postgres",
        Dialect::Mysql => "mysql",
    };
    let mut args = vec![format!("database = \"{}\"", database)];
    if let Some(model) = model {
        args.push(format!("model = \"{}\"", model));
    }
    if timestamps {
        args.push("timestamps".to_string());
    }
    if soft_delete {
        args.push("soft_delete".to_string());
    }
    format!("\n    #[sqlx_model({})]", args.join(", "))
}

fn render_model(name: &str, openapi: bool, sqlx: bool, timestamps: bool, fields: &[Field], dialect: Dialect) -> String {
    let (imports, derives, configure) = if openapi {
        (OPENAPI_IMPORTS, OPENAPI_DERIVES, OPENAPI_CONFIGURE)
    } else {
        (ENDPOINT_IMPORTS, "", ENDPOINT_CONFIGURE)
    };
    let field_lines = struct_fields(fields);
    // keeps the blank line of the empty creatable struct
    let new_fields = if fields.is_empty() { "\n" } else { &field_lines };
    let chrono_imports = chrono_imports(fields, timestamps);
    let (model_fields, updatable_fields) = if timestamps {
        let timestamp = |name: &str| Field { name: name.to_string(), ty: TIMESTAMP_TYPE.to_string(), references: None, unique: false, length: None };
        (
            field_lines.clone() + &struct_fields(&TIMESTAMP_COLUMNS.map(timestamp)),
            field_lines.clone() + &struct_fields(&[timestamp("updated_at")]),
        )
    } else {
        (field_lines.clone(), field_lines.clone())
    };
    // with --sqlx, the derives implement the model traits, left to fill otherwise
    let tpl = if sqlx {
        MODEL_TPL
            .replace("{trait_imports}", SQLX_TRAIT_IMPORTS)
            .replace("{model_impl}", "")
            .replace("{new_impl}", "")
            .replace("{updatable_impl}", "")
            .replace("{result_types}", "")
            .replace("{list_query_fields}", LIST_QUERY_FIELDS)
            .replace("{model_derives}", ", sqlx::FromRow, HttpFindListDelete, SqlxModel")
            .replace("{new_derives}", ", HttpCreate, SqlxNewModel")
            .replace("{updatable_derives}", ", sqlx::FromRow, HttpUpdate, SqlxUpdatableModel")
            .replace("{model_sqlx}", &sqlx_model_attribute(dialect, None, timestamps, timestamps))
            .replace("{new_sqlx}", &sqlx_model_attribute(dialect, Some(name), timestamps, false))
            .replace("{updatable_sqlx}", &sqlx_model_attribute(dialect, None, timestamps, timestamps))
    } else {
        let [find_body, list_body, delete_body, save_body, update_body] = EMPTY_BODIES;
        MODEL_TPL
            .replace("{trait_imports}", TRAIT_IMPORTS)
            .replace("{model_impl}", &MODEL_IMPL.replace("{find_body}", find_body).replace("{list_body}", list_body).replace("{delete_body}", delete_body))
            .replace("{new_impl}", &NEW_IMPL.replace("{save_body}", save_body))
            .replace("{updatable_impl}", &UPDATABLE_IMPL.replace("{update_body}", update_body))
            .replace("{result_types}", "\n    pub type ListResult = Vec<{entity}>;\n    pub type DeleteResult = {entity};")
            .replace("{list_query_fields}", "")
            .replace("{model_derives}", ", HttpFindListDelete")
            .replace("{new_derives}", ", HttpCreate")
            .replace("{updatable_derives}", ", HttpUpdate")
            .replace("{model_sqlx}", "")
            .replace("{new_sqlx}", "")
            .replace("{updatable_sqlx}", "")
    };
    tpl.replace("{model_fields}", &model_fields)
        .replace("{updatable_fields}", &updatable_fields)
        .replace("{chrono_imports}", &chrono_imports)
        .replace("{new_fields}", new_fields)
        .replace("{openapi_imports}", imports)
        .replace("{openapi_derives}", derives)
        .replace("{openapi_configure}", configure)
        .replace("{id_type}", ID_TYPE)
        .replace("{entity}", name)
        .replace("{entity_lower_case}", &name.to_lowercase())
}

const TRAIT_IMPORTS: &str = "
        HttpCreate,
        HttpFindListDelete,
        HttpUpdate,
        Model,
        NewModel,
        UpdatableModel,
        octopux_info,
        anyhow::Result,
        async_trait,";

const SQLX_TRAIT_IMPORTS: &str = "
        HttpCreate,
        HttpFindListDelete,
        HttpUpdate,
        SqlxModel,
        SqlxNewModel,
        SqlxUpdatableModel,
        octopux_info,";

const MODEL_IMPL: &str = r#"

    #[async_trait]
    impl Model<Id, FindQuery, ListQuery, ListResult, DeleteQuery, DeleteResult, AppState> for {entity} {
        async fn find(id: Id, _query: &FindQuery, _state: &AppState) -> Result<Box<{entity}>> {
{find_body}
        }
        async fn list(_query: &ListQuery, _state: &AppState) -> Result<ListResult> {
{list_body}
        }
        async fn delete(mut self: Self, _query: &DeleteQuery, _state: &AppState) -> Result<DeleteResult> {
{delete_body}
        }
    }"#;

const NEW_IMPL: &str = r#"
    #[async_trait]
    impl NewModel<{entity}, SaveQuery, AppState> for New{entity} {
        async fn save(self: Self, _query: &SaveQuery, _state: &AppState) -> Result<{entity}> {
{save_body}
        }
    }"#;

const UPDATABLE_IMPL: &str = r#"
    #[async_trait]
    impl UpdatableModel<Updatable{entity}, UpdateQuery, AppState> for Updatable{entity} {
        async fn update(mut self: Self, _query: &UpdateQuery, _state: &AppState) -> Result<Updatable{entity}> {
{update_body}
        }
    }"#;

const MODEL_TPL: &str = r#"
    // The application state, declared (or re-exported) at the root of the crate
    use crate::AppState;
    use serde::{Serialize, Deserialize};
    use octopux::{{trait_imports}
    };{chrono_imports}{openapi_imports}

    #[derive(Default, Deserialize{openapi_derives})]
    pub struct FindQuery {}
    #[derive(Deserialize{openapi_derives})]
    pub struct ListQuery {{list_query_fields}}
    #[derive(Deserialize{openapi_derives})]
    pub struct DeleteQuery {}{result_types}
    #[derive(Deserialize{openapi_derives})]
    pub struct SaveQuery {}
    #[derive(Deserialize{openapi_derives})]
    pub struct UpdateQuery {}
    pub type Id = {id_type};

    #[derive(Default, Serialize, Deserialize{openapi_derives}{model_derives})]
    #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]{model_sqlx}
    #[octopux_info(path = "{entity_lower_case}")]
    pub struct {entity} {
        pub id: Id,{model_fields}
    }{model_impl}

    #[derive(Serialize, Deserialize{openapi_derives}{new_derives})]
    #[http_create(SaveQuery, AppState)]{new_sqlx}
    pub struct New{entity} {{new_fields}
    }{new_impl}

    #[derive(Serialize, Deserialize{openapi_derives}{updatable_derives})]
    #[http_update(Id, UpdateQuery, {entity}, FindQuery, AppState)]{updatable_sqlx}
    pub struct Updatable{entity} {
        pub id: Id,{updatable_fields}
    }{updatable_impl}{openapi_configure}
    "#;

// Plural of the last word of a snake_case name: `book` gives `books`, `category` gives `categories`
fn pluralize(name: &str) -> String {
    let consonant_y = name.ends_with('y') && !name[..name.len() - 1].ends_with(['a', 'e', 'i', 'o', 'u']);
    if consonant_y {
        format!("{}ies", &name[..name.len() - 1])
    } else if ["s", "x", "z", "ch", "sh"].iter().any(|end| name.ends_with(end)) {
        format!("{}es", name)
    } else {
        format!("{}s", name)
    }
}

// `favorite_books` gives `FavoriteBooks`
fn to_camel_case(name: &str) -> String {
    name.split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or(String::new(), |c| c.to_uppercase().chain(chars).collect())
        })
        .collect()
}

#[derive(Debug, PartialEq)]
struct Through {
    model: String,
    // column of the join table referencing the child
    child_key: String,
}

// A has-many relation of `parent`, its children referencing it by `foreign_key`,
// directly or through a join model
#[derive(Debug, PartialEq)]
struct Relation {
    parent: String,
    child: String,
    // last segment of the route
    name: String,
    foreign_key: String,
    through: Option<Through>,
}

impl Relation {
    fn new(parent: String, child: String, name: Option<String>, foreign_key: Option<String>, through: Option<String>, child_key: Option<String>) -> Relation {
        let child_snake = to_snake_case(&child);
        Relation {
            name: name.unwrap_or_else(|| pluralize(&child_snake)),
            foreign_key: foreign_key.unwrap_or_else(|| format!("{}_id", to_snake_case(&parent))),
            through: through.map(|model| Through {
                model,
                child_key: child_key.unwrap_or_else(|| format!("{}_id", child_snake)),
            }),
            parent,
            child,
        }
    }

    // Module of the relation, next to the models: `project_books`
    fn module(&self) -> String {
        format!("{}_{}", to_snake_case(&self.parent), self.name)
    }

    // Type the `HasMany` trait is implemented on: `ProjectBooks`
    fn type_name(&self) -> String {
        format!("{}{}", self.parent, to_camel_case(&self.name))
    }

    // Table and column holding the foreign key, to index
    fn foreign_key_column(&self) -> (String, &str) {
        let table = self.through.as_ref().map_or(&self.child, |through| &through.model);
        (table.to_lowercase(), &self.foreign_key)
    }
}

// Body of list_related: a page of the children, and the lookup of the parent when the page is empty,
// to answer 404 for an unknown parent, `timestamps` skips the soft deleted rows
fn relation_sqlx_body(relation: &Relation, timestamps: bool, dialect: Dialect) -> String {
    let parent = relation.parent.to_lowercase();
    let child = relation.child.to_lowercase();
    let placeholders = dialect.placeholders(3);
    let select = match &relation.through {
        None => format!(
            "SELECT * FROM {} WHERE {} = {}{} ORDER BY id LIMIT {} OFFSET {}",
            child,
            relation.foreign_key,
            placeholders[0],
            if timestamps { " AND deleted_at IS NULL" } else { "" },
            placeholders[1],
            placeholders[2]
        ),
        Some(through) => {
            let join = through.model.to_lowercase();
            format!(
                "SELECT {child}.* FROM {child} JOIN {join} ON {join}.{child_key} = {child}.id WHERE {join}.{fk} = {p1}{live} ORDER BY {child}.id LIMIT {p2} OFFSET {p3}",
                child = child,
                join = join,
                child_key = through.child_key,
                fk = relation.foreign_key,
                p1 = placeholders[0],
                live = if timestamps { format!(" AND {}.deleted_at IS NULL AND {}.deleted_at IS NULL", child, join) } else { String::new() },
                p2 = placeholders[1],
                p3 = placeholders[2],
            )
        }
    };
    let lookup = format!(
        "SELECT id FROM {} WHERE id = {}{}",
        parent,
        placeholders[0],
        if timestamps { " AND deleted_at IS NULL" } else { "" }
    );
    format!(
        "{}\n{}
            // the parent is only looked up when the page is empty
            if models.is_empty() {{
                let parent = sqlx::query_scalar::<_, Id>(
                    \"{}\",
                )
                .bind(id)
                .fetch_optional(&state.pool)
                .await?;
                if parent.is_none() {{
                    return Ok(None);
                }}
            }}
            Ok(Some(models))",
        LIST_PAGINATION,
        sqlx_fetch_all(
            "models",
            &relation.child,
            &select,
            &["id".to_string(), "limit".to_string(), "offset".to_string()],
        ),
        lookup
    )
}

fn render_relation(relation: &Relation, openapi: bool, sqlx: bool, timestamps: bool, dialect: Dialect) -> String {
    let (imports, derives, service_config, endpoint) = if openapi {
        (
            "\n    use apistos::ApiComponent;\n    use schemars::JsonSchema;\n    use octopux::gen_documented_relation_endpoint;",
            OPENAPI_DERIVES,
            "apistos::web::ServiceConfig",
            "gen_documented_relation_endpoint",
        )
    } else {
        (
            "\n    use octopux::gen_relation_endpoint;",
            "",
            "actix_web::web::ServiceConfig",
            "gen_relation_endpoint",
        )
    };
    let (body, query, state, limits) = if sqlx {
        (relation_sqlx_body(relation, timestamps, dialect), "query", "state", RELATION_LIMITS)
    } else {
        (
            "            // list the children of the parent `id`, None when it does not exist".to_string(),
            "_query",
            "_state",
            "",
        )
    };
    RELATION_TPL
        .replace("{body}", &body)
        .replace("{query}", query)
        .replace("{state}", state)
        .replace("{list_limits}", limits)
        .replace("{openapi_imports}", imports)
        .replace("{openapi_derives}", derives)
        .replace("{service_config}", service_config)
        .replace("{endpoint}", endpoint)
        .replace("{relation}", &relation.type_name())
        .replace("{relation_name}", &relation.name)
        .replace("{module}", &relation.module())
        .replace("{parent_module}", &to_snake_case(&relation.parent))
        .replace("{child_module}", &to_snake_case(&relation.child))
        .replace("{parent_lower_case}", &relation.parent.to_lowercase())
        .replace("{parent}", &relation.parent)
        .replace("{child}", &relation.child)
}

// Index of the foreign key, which every page of the relation filters on
fn render_relation_migration(relation: &Relation, dialect: Dialect) -> String {
    let (table, column) = relation.foreign_key_column();
    let if_not_exists = if dialect == Dialect::Mysql { "" } else { "IF NOT EXISTS " };
    format!("CREATE INDEX {}{}_{}_idx ON {} ({});\n", if_not_exists, table, column, table, column)
}

const RELATION_TPL: &str = r#"
    // The application state, declared (or re-exported) at the root of the crate
    use crate::AppState;
    use crate::{parent_module}::{{parent}, Id};
    use crate::{child_module}::{child};
    use serde::Deserialize;
    use octopux::{
        HasMany,
        anyhow::Result,
        async_trait,
    };{openapi_imports}

    #[derive(Deserialize{openapi_derives})]
    pub struct {relation}Query {
        /// Number of rows to skip
        pub offset: Option<usize>,
        /// Maximum number of rows to return (20 by default, 100 at most)
        pub limit: Option<usize>,
    }{list_limits}

    /// The {relation_name} of a {parent_lower_case}, served on `GET /{parent_lower_case}/{id}/{relation_name}`
    pub struct {relation};

    #[async_trait]
    impl HasMany for {relation} {
        type Parent = {parent};
        type Id = Id;
        type Query = {relation}Query;
        type Result = Vec<{child}>;
        type State = AppState;
        const RELATION: &'static str = "{relation_name}";

        async fn list_related(id: Id, {query}: &{relation}Query, {state}: &AppState) -> Result<Option<Vec<{child}>>> {
{body}
        }
    }

    // Registers the route of the {relation_name} of a {parent_lower_case}, to mount with `.configure({module}::configure)`
    // in the same scope as the {parent_lower_case} routes
    pub fn configure(cfg: &mut {service_config}) {
        {endpoint}!({relation})(cfg)
    }
    "#;

// Writes src/main.rs and src/helpers.rs under `root`, only if src/helpers.rs does not exist,
// an existing src/main.rs (such as the one of `cargo init`) is only overwritten once confirmed
fn bootstrap<R: BufRead, W: Write>(root: &Path, openapi: bool, input: &mut R, output: &mut W) -> Result<(), Error> {
    let main_content = if openapi { BOOTSTRAP_OPENAPI_MAIN } else { BOOTSTRAP_MAIN };
    let files = [("main.rs", main_content), ("helpers.rs", BOOTSTRAP_HELPERS)];
    let src = root.join("src");
    let helpers = src.join("helpers.rs");
    if helpers.exists() {
        eprintln!("{}", failure(&format!("{} already exists, project not bootstrapped", helpers.display())));
        process::exit(1);
    }
    let main = src.join("main.rs");
    if main.exists() && !confirm(input, output, &format!("{} already exists, overwrite it with the generated one?", main.display()))? {
        eprintln!("{}", failure(&format!("{} kept, project not bootstrapped", main.display())));
        process::exit(1);
    }
    fs::create_dir_all(&src)?;
    for (name, content) in files {
        fs::write(src.join(name), with_header("//", content))?;
    }
    println!(
        "{}",
        success(&format!(
            "Successfully bootstrapped src/main.rs and src/helpers.rs, generate a model with `octopux generate-model --name <Model>{}`, then declare it with `mod <model>;` and mount it with `.configure(<model>::configure)` in the v1 scope of src/main.rs",
            if openapi { " --openapi" } else { "" }
        ))
    );
    Ok(())
}

// `cargo add` arguments of the dependencies of the bootstrapped files and of the generated models,
// octopux is taken from the tag of the CLI version in its repository, so that the generated code matches its macros,
// apistos and schemars are only added with --openapi
fn bootstrap_dependencies(openapi: bool) -> Vec<Vec<String>> {
    let tag = format!("v{}", env!("CARGO_PKG_VERSION"));
    let features = if openapi { "openapi,sqlx" } else { "sqlx" };
    let mut deps = vec![
        vec!["octopux", "--git", env!("CARGO_PKG_REPOSITORY"), "--tag", tag.as_str(), "--features", features],
        vec!["actix-web@4"],
    ];
    if openapi {
        deps.push(vec!["apistos@0.9", "--features", "chrono,swagger-ui"]);
        deps.push(vec!["apistos-schemars@0.8", "--rename", "schemars"]);
    }
    deps.extend([
        vec!["serde@1", "--features", "derive"],
        vec!["chrono@0.4", "--features", "serde"],
        vec!["sqlx@0.9", "--no-default-features", "--features", "runtime-tokio,sqlite,chrono,macros,migrate"],
    ]);
    deps.iter().map(|args| args.iter().map(|a| a.to_string()).collect()).collect()
}

// Only an explicit yes accepts, an empty answer or the end of the input refuses
fn confirm<R: BufRead, W: Write>(input: &mut R, output: &mut W, message: &str) -> Result<bool, Error> {
    let answer = prompt(input, output, &format!("{} {} {} ", cyan("?"), bold(&highlight(message)), dim("(y/N)")))?;
    Ok(matches!(answer.as_deref().map(str::to_lowercase).as_deref(), Some("y" | "yes")))
}

// Runs `cargo add` in `root` for each dependency of the bootstrapped project
fn install_dependencies(root: &Path, openapi: bool) -> Result<(), Error> {
    if !root.join("Cargo.toml").exists() {
        eprintln!("{}", failure(&format!("No Cargo.toml in {}, dependencies not installed, create the crate with `cargo init` first", root.display())));
        process::exit(1);
    }
    for args in bootstrap_dependencies(openapi) {
        let status = process::Command::new("cargo").arg("add").args(&args).current_dir(root).status()?;
        if !status.success() {
            eprintln!("{}", failure(&format!("`cargo add {}` failed, remaining dependencies not installed", args.join(" "))));
            process::exit(1);
        }
    }
    println!("{}", success("Successfully installed the dependencies"));
    Ok(())
}

fn main() -> Result<(), Error> {
    let cli = Cli::from_args();
    COLOR.store(
        io::stdout().is_terminal() && io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none(),
        Ordering::Relaxed,
    );
    if cli.bootstrap {
        let root = std::env::current_dir()?;
        let mut input = io::stdin().lock();
        bootstrap(&root, cli.openapi, &mut input, &mut io::stdout())?;
        if confirm(&mut input, &mut io::stdout(), "Install the dependencies with `cargo add`?")? {
            install_dependencies(&root, cli.openapi)?;
        }
    }
    match cli.cmd {
        Some(opt) => run(opt),
        None if cli.bootstrap => Ok(()),
        None => {
            Cli::clap().print_help().map_err(|e| Error::other(e.to_string()))?;
            println!();
            Ok(())
        }
    }
}

fn run(opt: Opt) -> Result<(), Error> {
    match opt {
        Opt::GenerateModel { name, openapi, fields, sqlx, migration, foreign_keys, unique, sqlite: _, postgres, mysql, timestamps, force } => {
            let dialect = Dialect::from_flags(postgres, mysql);
            let module = to_snake_case(&name);
            let path = source_path(&std::env::current_dir()?, &format!("{}.rs", module));
            let overwrites = Path::new(&path).exists();
            refuse_overwrite(&path, force, &format!("model {}", name));
            let fields_asked = fields;
            let fields = if fields {
                let strict = if migration { Some(dialect) } else { None };
                let tables = if foreign_keys { known_tables(dialect)? } else { Vec::new() };
                let table = name.to_lowercase();
                let references = foreign_keys.then_some((table.as_str(), tables.as_slice()));
                read_fields(&mut io::stdin().lock(), &mut io::stdout(), timestamps, strict, references, unique)?
            } else {
                Vec::new()
            };
            // the INSERT and UPDATE queries need at least one column
            if sqlx && fields.is_empty() {
                eprintln!("{}", failure(&format!("--sqlx needs at least one field, model {} not generated", name)));
                process::exit(1);
            }
            let sql = if migration {
                Some(render_migration(&name, &fields, timestamps, dialect).unwrap_or_else(|e| {
                    eprintln!("{}", failure(&format!("{}, model {} not generated", e, name)));
                    process::exit(1);
                }))
            } else {
                None
            };
            let migration = match sql {
                Some(sql) => Some((migration_path(&format!("create_{}", name.to_lowercase()))?, sql)),
                None => None,
            };
            // the fields were entered interactively, nothing is written before the recap is confirmed
            if fields_asked {
                let mut files = vec![(path.clone(), overwrites)];
                files.extend(migration.iter().map(|(p, _)| (p.display().to_string(), false)));
                if !confirm_save(&mut io::stdin().lock(), &mut io::stdout(), &files)? {
                    eprintln!("{}", failure(&format!("Nothing saved, model {} not generated", name)));
                    process::exit(1);
                }
            }
            fs::write(&path, with_header("//", &render_model(&name, openapi, sqlx, timestamps, &fields, dialect)))?;
            println!("{}", success(&format!("Successfully generated model {}, declare it with `mod {};`", path, module)));
            if let Some((migration, sql)) = migration {
                write_migration(&migration, &sql)?;
                if overwrites {
                    println!(
                        "{}",
                        warning(&format!("The migration creates the {} table only if it does not exist yet", name.to_lowercase()))
                    );
                }
            }
            Ok(())
        }
        Opt::GenerateRelation { parent, child, name, foreign_key, through, child_key, openapi, sqlx, migration, sqlite: _, postgres, mysql, timestamps, force } => {
            let dialect = Dialect::from_flags(postgres, mysql);
            let relation = Relation::new(parent, child, name, foreign_key, through, child_key);
            let columns = [Some(&relation.name), Some(&relation.foreign_key), relation.through.as_ref().map(|t| &t.child_key)];
            if let Some(invalid) = columns.into_iter().flatten().find(|c| !is_field_name(c)) {
                eprintln!("{}", failure(&format!("`{}` is not a valid name, use snake_case, relation not generated", invalid)));
                process::exit(1);
            }
            let module = relation.module();
            let path = source_path(&std::env::current_dir()?, &format!("{}.rs", module));
            refuse_overwrite(&path, force, "relation");
            fs::write(&path, with_header("//", &render_relation(&relation, openapi, sqlx, timestamps, dialect)))?;
            println!(
                "{}",
                success(&format!(
                    "Successfully generated relation {}, declare it with `mod {};` and mount it with `.configure({}::configure)` in the scope of the {} routes",
                    path, module, module, relation.parent.to_lowercase()
                ))
            );
            if migration {
                let (table, column) = relation.foreign_key_column();
                write_migration(&migration_path(&format!("index_{}_{}", table, column))?, &render_relation_migration(&relation, dialect))?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        migration_timestamp, migrations_dir, source_path, parse_field_type, pluralize, read_fields, render_migration, render_model, render_relation, render_relation_migration, to_camel_case,
        to_snake_case, bootstrap, bootstrap_dependencies, changes_summary, confirm, confirm_save, generated_header, migration_tables, parse_tables, postgres_column_type, url_dialect,
        Cli, Column, Dialect, Field, Opt, Reference, Relation, Table, Through, LOGO,
    };
    use structopt::StructOpt;

    fn field(name: &str, ty: &str) -> Field {
        Field { name: name.into(), ty: ty.into(), references: None, unique: false, length: None }
    }

    fn column(name: &str, sql_type: &str, unique: bool) -> Column {
        Column { name: name.into(), sql_type: sql_type.into(), unique }
    }

    fn author_table() -> Table {
        Table { name: "author".into(), columns: vec![column("id", "INT8", true), column("email", "TEXT", true), column("name", "TEXT", false)] }
    }

    #[test]
    fn parse_tables_reads_the_created_tables() {
        let sql = super::with_header("--", "CREATE TABLE IF NOT EXISTS author (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    score DOUBLE PRECISION,
    price DECIMAL(10, 2) DEFAULT 0,
    email VARCHAR(255) UNIQUE NOT NULL
);
create table \"Book\" (code TEXT, author_id INT8, PRIMARY KEY (code), FOREIGN KEY (author_id) REFERENCES author (id));
CREATE INDEX book_idx ON book (code);
");
        assert_eq!(parse_tables(&sql), vec![
            Table {
                name: "author".into(),
                columns: vec![
                    column("id", "BIGSERIAL", true),
                    column("name", "TEXT", false),
                    column("score", "DOUBLE PRECISION", false),
                    column("price", "DECIMAL(10, 2)", false),
                    column("email", "VARCHAR(255)", true),
                ],
            },
            Table { name: "Book".into(), columns: vec![column("code", "TEXT", true), column("author_id", "INT8", false)] },
        ]);
    }

    #[test]
    fn migration_tables_keep_the_first_definition_of_a_table() {
        let dir = std::env::temp_dir().join(format!("octopux-migration-tables-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("2_create_book.sql"), "CREATE TABLE IF NOT EXISTS book (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL);").unwrap();
        std::fs::write(dir.join("1_create_book.sql"), "CREATE TABLE IF NOT EXISTS book (id INTEGER PRIMARY KEY AUTOINCREMENT);").unwrap();
        std::fs::write(dir.join("3_create_author.down.sql"), "CREATE TABLE author (id INTEGER);").unwrap();
        let tables = migration_tables(&dir);
        assert_eq!(tables, vec![Table { name: "book".into(), columns: vec![column("id", "INTEGER", true)] }]);
        assert!(migration_tables(&dir.join("missing")).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn read_fields_asks_for_the_referenced_table_and_column() {
        let tables = [author_table()];
        // table by number and default column, table and column by name, no reference, unknown table then self reference
        let input = "author_id:i64\n1\n\nwriter_email\n\n100\nAUTHOR\nemail\ntitle\n\n\n\nparent_id:i64\nnope\n2\n\n\n";
        let mut output = Vec::new();
        let fields = read_fields(&mut input.as_bytes(), &mut output, false, Some(Dialect::Postgres), Some(("book", &tables)), false).unwrap();
        let reference = |table: &str, column: &str| Some(Reference { table: table.into(), column: column.into() });
        assert_eq!(fields, vec![
            Field { references: reference("author", "id"), ..field("author_id", "i64") },
            Field { references: reference("author", "email"), length: Some(100), ..field("writer_email", "String") },
            field("title", "String"),
            Field { references: reference("book", "id"), ..field("parent_id", "i64") },
        ]);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("1) author  2) book (this model)"));
        assert!(output.contains("1) id INT8 (unique)  2) email TEXT (unique)  3) name TEXT"));
        assert!(output.contains("? Column of `author` referenced by `author_id` › (number or name) [id] "));
        assert!(output.contains("`nope` is not a known table, pick 1 to 2"));
        // the model table has its id and the fields declared before
        assert!(output.contains("1) id BIGSERIAL (unique)  2) author_id INT8  3) writer_email VARCHAR(100)  4) title VARCHAR(255)"));
        assert!(output.contains("writer_email: String (length 100) → author (email)"));
        assert!(output.contains("author_id: i64 → author (id)"));
        assert!(!output.contains("may be refused"));
    }

    #[test]
    fn read_fields_warns_about_references_the_database_refuses() {
        let tables = [author_table()];
        let mut output = Vec::new();
        let input = "author_id:i32\nauthor\n\nauthor_name:String\n\n1\nname\n\n";
        let fields = read_fields(&mut input.as_bytes(), &mut output, false, Some(Dialect::Postgres), Some(("book", &tables)), false).unwrap();
        assert_eq!(fields.len(), 2);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("`author_id` is INT4 and `author.id` is INT8, the foreign key may be refused"));
        assert!(output.contains("`author.name` is neither a primary key nor unique"));
        // SQLite does not check the types
        let mut output = Vec::new();
        read_fields(&mut "author_id:i32\n1\n\n\n".as_bytes(), &mut output, false, Some(Dialect::Sqlite), Some(("book", &tables)), false).unwrap();
        assert!(!String::from_utf8(output).unwrap().contains("may be refused"));
    }

    #[test]
    fn migration_declares_the_foreign_keys() {
        let fields = vec![
            Field { references: Some(Reference { table: "author".into(), column: "id".into() }), ..field("author_id", "i64") },
            field("title", "String"),
            Field { references: Some(Reference { table: "book".into(), column: "id".into() }), ..field("parent_id", "Option<i64>") },
        ];
        assert_eq!(render_migration("Book", &fields, true, Dialect::Mysql).unwrap(), "CREATE TABLE IF NOT EXISTS book (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    author_id BIGINT NOT NULL,
    title VARCHAR(255) NOT NULL,
    parent_id BIGINT,
    created_at DATETIME(6),
    updated_at DATETIME(6),
    deleted_at DATETIME(6),
    FOREIGN KEY (author_id) REFERENCES author (id),
    FOREIGN KEY (parent_id) REFERENCES book (id)
);
");
    }

    #[test]
    fn foreign_keys_flag_requires_migration() {
        let parse = |flags: &[&str]| {
            let mut args = vec!["octopux", "generate-model", "--name", "Book", "--fields"];
            args.extend(flags);
            Opt::from_iter_safe(&args)
        };
        assert!(parse(&["--foreign-keys"]).is_err());
        assert!(parse(&["--foreign-keys", "--migration"]).is_ok());
    }

    #[test]
    fn read_fields_asks_whether_the_columns_are_unique() {
        // no reference and unique, not unique by default, then a self reference proposing the unique field as unique
        let input = "email\n\n\n\ny\nname\n\n\n\n\nparent_email\n\n\n1\nemail\nno\n\n";
        let mut output = Vec::new();
        let fields = read_fields(&mut input.as_bytes(), &mut output, false, Some(Dialect::Postgres), Some(("author", &[])), true).unwrap();
        let unique = |name: &str| Field { unique: true, ..field(name, "String") };
        assert_eq!(fields, vec![
            unique("email"),
            field("name", "String"),
            Field { references: Some(Reference { table: "author".into(), column: "email".into() }), ..field("parent_email", "String") },
        ]);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("? Is `email` unique › (y/N) "));
        assert!(output.contains("1) id BIGSERIAL (unique)  2) email VARCHAR(255) (unique)  3) name VARCHAR(255)"));
        assert!(output.contains("email: String (unique)"));
        assert!(!output.contains("neither a primary key nor unique"));
    }

    #[test]
    fn read_fields_warns_about_unique_columns_mysql_refuses() {
        let mut output = Vec::new();
        let fields = read_fields(&mut "hash:Vec<u8>\ny\nemail\n\n\ny\n\n".as_bytes(), &mut output, false, Some(Dialect::Mysql), None, true).unwrap();
        assert!(fields.iter().all(|f| f.unique));
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("MySQL refuses a unique index on the BLOB column `hash`"));
        assert!(!output.contains("column `email`"));
    }

    #[test]
    fn read_fields_asks_for_the_varchar_lengths() {
        // too long for MySQL, zero, then a length, the default length, no length asked for a TEXT column
        let input = "title\n\n20000\n0\n80\nsummary:String?\n\n\n";
        let mut output = Vec::new();
        let fields = read_fields(&mut input.as_bytes(), &mut output, false, Some(Dialect::Mysql), None, false).unwrap();
        assert_eq!(fields, vec![Field { length: Some(80), ..field("title", "String") }, field("summary", "Option<String>")]);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("? Length of `title` › (VARCHAR, 1 to 16383) [255] "));
        assert!(output.contains("`20000` is not a MySQL VARCHAR length, pick 1 to 16383"));
        assert!(output.contains("`0` is not a MySQL VARCHAR length"));
        assert_eq!(render_migration("Post", &fields, false, Dialect::Mysql).unwrap(), "CREATE TABLE IF NOT EXISTS post (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    title VARCHAR(80) NOT NULL,
    summary VARCHAR(255)
);
");
        let mut output = Vec::new();
        read_fields(&mut "title\n\n\n".as_bytes(), &mut output, false, Some(Dialect::Sqlite), None, false).unwrap();
        assert!(!String::from_utf8(output).unwrap().contains("Length of"));
    }

    #[test]
    fn migration_declares_the_unique_constraints() {
        let fields = vec![
            Field { unique: true, ..field("email", "String") },
            field("title", "String"),
            Field { unique: true, references: Some(Reference { table: "author".into(), column: "id".into() }), ..field("author_id", "i64") },
        ];
        assert_eq!(render_migration("Book", &fields, false, Dialect::Postgres).unwrap(), "CREATE TABLE IF NOT EXISTS book (
    id BIGSERIAL PRIMARY KEY,
    email VARCHAR(255) NOT NULL,
    title VARCHAR(255) NOT NULL,
    author_id INT8 NOT NULL,
    UNIQUE (email),
    UNIQUE (author_id),
    FOREIGN KEY (author_id) REFERENCES author (id)
);
");
    }

    #[test]
    fn unique_flag_requires_migration() {
        let parse = |flags: &[&str]| {
            let mut args = vec!["octopux", "generate-model", "--name", "Book", "--fields"];
            args.extend(flags);
            Opt::from_iter_safe(&args)
        };
        assert!(parse(&["--unique"]).is_err());
        assert!(parse(&["--unique", "--migration"]).is_ok());
    }

    #[test]
    fn database_url_types_match_the_sqlx_types() {
        assert_eq!(url_dialect("postgres://user@localhost/db"), Some(Dialect::Postgres));
        assert_eq!(url_dialect("sqlite://data.db?mode=rwc"), Some(Dialect::Sqlite));
        assert_eq!(url_dialect("mariadb://localhost/db"), Some(Dialect::Mysql));
        assert_eq!(url_dialect("redis://localhost"), None);
        assert_eq!(postgres_column_type("int8"), "INT8");
        assert_eq!(postgres_column_type("_text"), "TEXT[]");
        assert!(Dialect::Postgres.same_column_type("INT8", "bigserial"));
        assert!(Dialect::Postgres.same_column_type("FLOAT8", "DOUBLE  PRECISION"));
        assert!(!Dialect::Postgres.same_column_type("INT4", "INT8"));
        assert!(Dialect::Postgres.same_column_type("VARCHAR(255)", "text"));
        assert!(Dialect::Postgres.same_column_type("VARCHAR(255)", "VARCHAR"));
        assert!(!Dialect::Postgres.same_column_type("VARCHAR(255)", "INT8"));
        assert!(Dialect::Mysql.same_column_type("BIGINT", "bigint"));
        assert!(!Dialect::Mysql.same_column_type("INT", "BIGINT"));
    }

    #[test]
    fn default_model_has_no_openapi_derives_and_an_undocumented_configure() {
        let model = render_model("Project", false, false, false, &[], Dialect::Sqlite);
        assert!(!model.contains("JsonSchema"));
        assert!(!model.contains("ApiComponent"));
        assert!(model.contains("use octopux::gen_endpoint;"));
        assert!(model.contains("pub fn configure(cfg: &mut actix_web::web::ServiceConfig) {\n        gen_endpoint!(Project, NewProject, UpdatableProject)(cfg)\n    }"));
        assert!(model.contains("`.configure(project::configure)`"));
        assert!(!model.contains("gen_documented_endpoint"));
        assert!(!model.contains("{openapi"));
        assert!(model.contains("#[octopux_info(path = \"project\")]"));
    }

    #[test]
    fn openapi_model_derives_schemas_on_every_route_type() {
        let model = render_model("Project", true, false, false, &[], Dialect::Sqlite);
        assert!(model.contains("use apistos::ApiComponent;"));
        assert!(model.contains("use schemars::JsonSchema;"));
        assert!(model.contains("use octopux::gen_documented_endpoint;"));
        assert!(model.contains("pub fn configure(cfg: &mut apistos::web::ServiceConfig) {"));
        assert!(model.contains("gen_documented_endpoint!(Project, NewProject, UpdatableProject)(cfg)"));
        // 5 query structs + Project, NewProject and UpdatableProject
        assert_eq!(model.matches(", JsonSchema, ApiComponent").count(), 8);
        assert!(!model.contains("{openapi"));
        assert!(!model.contains("{entity"));
    }

    #[test]
    fn model_only_depends_on_octopux_and_the_app_state() {
        let model = render_model("Project", false, false, false, &[], Dialect::Sqlite);
        assert!(model.contains("use crate::AppState;"));
        assert!(model.contains("        octopux_info,\n        anyhow::Result,\n        async_trait,\n    };"));
        assert!(!model.contains("octopux_derive"));
        assert!(!model.contains("use anyhow"));
        assert!(!model.contains("use async_trait"));
    }

    #[test]
    fn model_without_fields_keeps_empty_structs() {
        let model = render_model("Project", false, false, false, &[], Dialect::Sqlite);
        assert!(model.contains("pub type Id = i64;"));
        assert!(model.contains("pub struct Project {\n        pub id: Id,\n    }"));
        assert!(model.contains("pub struct NewProject {\n\n    }"));
        assert!(model.contains("pub struct UpdatableProject {\n        pub id: Id,\n    }"));
        assert!(!model.contains("_fields}"));
    }

    #[test]
    fn fields_are_added_to_every_model_struct() {
        let fields = vec![
            field("title", "String"),
            field("stars", "i32"),
        ];
        let model = render_model("Project", false, false, false, &fields, Dialect::Sqlite);
        assert!(model.contains("pub struct Project {\n        pub id: Id,\n        pub title: String,\n        pub stars: i32,\n    }"));
        assert!(model.contains("pub struct NewProject {\n        pub title: String,\n        pub stars: i32,\n    }"));
        assert!(model.contains("pub struct UpdatableProject {\n        pub id: Id,\n        pub title: String,\n        pub stars: i32,\n    }"));
    }

    #[test]
    fn read_fields_prompts_until_empty_name() {
        // invalid names, default type, duplicate and reserved `id` are handled
        let input = "title\n\n1bad\nstars\ni32\ntitle\nid\n\n";
        let mut output = Vec::new();
        let fields = read_fields(&mut input.as_bytes(), &mut output, false, None, None, false).unwrap();
        assert_eq!(fields, vec![
            field("title", "String"),
            field("stars", "i32"),
        ]);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("`1bad` is not a valid field name"));
        assert!(output.contains("Field `title` is already declared"));
        assert!(output.contains("Field `id` is already declared"));
    }

    #[test]
    fn read_fields_converts_names_to_snake_case() {
        let input = "OptStr\n\nhttpCode\n\nfirst name\n\nlast-name\n\nopt_str\nID\n\n";
        let mut output = Vec::new();
        let fields = read_fields(&mut input.as_bytes(), &mut output, false, None, None, false).unwrap();
        let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["opt_str", "http_code", "first_name", "last_name"]);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("`OptStr` renamed to `opt_str`"));
        assert!(output.contains("Field `opt_str` is already declared"));
        assert!(output.contains("Field `id` is already declared"));
    }

    #[test]
    fn to_snake_case_splits_words_and_acronyms() {
        assert_eq!(to_snake_case("title"), "title");
        assert_eq!(to_snake_case("createdAt"), "created_at");
        assert_eq!(to_snake_case("CreatedAt"), "created_at");
        assert_eq!(to_snake_case("HTTPCode"), "http_code");
        assert_eq!(to_snake_case("userID"), "user_id");
        assert_eq!(to_snake_case("line2Total"), "line2_total");
        assert_eq!(to_snake_case("first  name"), "first_name");
        assert_eq!(to_snake_case("already_snake"), "already_snake");
    }

    #[test]
    fn read_fields_stops_at_end_of_input() {
        let mut output = Vec::new();
        let fields = read_fields(&mut "title\nString".as_bytes(), &mut output, false, None, None, false).unwrap();
        assert_eq!(fields, vec![field("title", "String")]);
    }

    #[test]
    fn field_type_can_be_picked_from_the_menu() {
        assert_eq!(parse_field_type(""), Some("String".into()));
        assert_eq!(parse_field_type("3"), Some("i64".into()));
        assert_eq!(parse_field_type("5"), Some("bool".into()));
        assert_eq!(parse_field_type("chrono::NaiveDate"), Some("chrono::NaiveDate".into()));
        assert_eq!(parse_field_type("0"), None);
        assert_eq!(parse_field_type("42"), None);
    }

    #[test]
    fn read_fields_asks_again_for_an_unknown_type_number() {
        let mut output = Vec::new();
        let fields = read_fields(&mut "done\n42\n5\n\n".as_bytes(), &mut output, false, None, None, false).unwrap();
        assert_eq!(fields, vec![field("done", "bool")]);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("1) String  2) i32"));
        assert!(output.contains("`42` is not in the list, pick 1 to 11"));
    }

    #[test]
    fn field_type_can_be_made_optional_with_a_question_mark() {
        assert_eq!(parse_field_type("?"), Some("Option<String>".into()));
        assert_eq!(parse_field_type("3?"), Some("Option<i64>".into()));
        assert_eq!(parse_field_type("NaiveDate?"), Some("Option<NaiveDate>".into()));
        assert_eq!(parse_field_type("Option<i32>?"), Some("Option<i32>".into()));
        assert_eq!(parse_field_type("42?"), None);
    }

    #[test]
    fn read_fields_accepts_inline_types_and_removes_the_last_field() {
        let input = "title:String\nstars:2?\nviews\n3\n-\nscore: f64\nbad:42\n5\n\n";
        let mut output = Vec::new();
        let fields = read_fields(&mut input.as_bytes(), &mut output, false, None, None, false).unwrap();
        assert_eq!(fields, vec![
            field("title", "String"),
            field("stars", "Option<i32>"),
            field("score", "f64"),
            field("bad", "bool"),
        ]);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Field `views` removed"));
        assert!(output.contains("`42` is not in the list, pick 1 to 11"));
        assert!(output.contains("4 fields declared"));
        assert!(output.contains("stars: Option<i32> (nullable)"));
        assert!(!output.contains('\x1b'));
    }

    #[test]
    fn sqlx_flag_requires_fields() {
        assert!(Opt::from_iter_safe(&["octopux", "generate-model", "--name", "Project", "--sqlx"]).is_err());
        assert!(Opt::from_iter_safe(&["octopux", "generate-model", "--name", "Project", "--sqlx", "--fields"]).is_ok());
    }

    #[test]
    fn sqlx_model_derives_the_model_traits() {
        let fields = vec![
            field("title", "String"),
            field("stars", "i32"),
        ];
        let model = render_model("Project", false, true, false, &fields, Dialect::Sqlite);
        assert!(!super::EMPTY_BODIES.iter().any(|body| model.contains(body)));
        assert!(!model.contains("impl "));
        assert!(!model.contains("async_trait"));
        assert!(!model.contains("SELECT"));
        assert!(model.contains("        SqlxModel,\n        SqlxNewModel,\n        SqlxUpdatableModel,\n        octopux_info,\n    };"));
        assert!(model.contains("#[derive(Default, Serialize, Deserialize, sqlx::FromRow, HttpFindListDelete, SqlxModel)]\n    #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]\n    #[sqlx_model(database = \"sqlite\")]\n    #[octopux_info(path = \"project\")]"));
        assert!(model.contains("#[derive(Serialize, Deserialize, HttpCreate, SqlxNewModel)]\n    #[http_create(SaveQuery, AppState)]\n    #[sqlx_model(database = \"sqlite\", model = \"Project\")]\n    pub struct NewProject {"));
        assert!(model.contains("#[derive(Serialize, Deserialize, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]\n    #[http_update(Id, UpdateQuery, Project, FindQuery, AppState)]\n    #[sqlx_model(database = \"sqlite\")]\n    pub struct UpdatableProject {"));
        assert!(!model.contains("{model") && !model.contains("_sqlx}") && !model.contains("_impl}"));
    }

    #[test]
    fn sqlx_model_paginates_the_list() {
        let fields = vec![field("title", "String")];
        let model = render_model("Project", false, true, false, &fields, Dialect::Sqlite);
        assert!(model.contains("pub struct ListQuery {\n        /// Number of rows to skip\n        pub offset: Option<usize>,"));
        assert!(model.contains("        pub limit: Option<usize>,\n    }"));
        // without --sqlx the list query stays empty
        let model = render_model("Project", false, false, false, &fields, Dialect::Sqlite);
        assert!(model.contains("pub struct ListQuery {}"));
        assert!(model.contains("async fn list(_query: &ListQuery, _state: &AppState)"));
    }

    #[test]
    fn sqlx_model_sets_timestamps_and_soft_deletes() {
        let fields = vec![field("title", "String")];
        let model = render_model("Project", false, true, true, &fields, Dialect::Sqlite);
        assert!(model.contains("use chrono::{DateTime, Utc};"));
        assert_eq!(model.matches("#[sqlx_model(database = \"sqlite\", timestamps, soft_delete)]").count(), 2);
        assert!(model.contains("#[sqlx_model(database = \"sqlite\", model = \"Project\", timestamps)]"));
    }

    #[test]
    fn sqlx_model_targets_the_database() {
        let fields = vec![field("title", "String")];
        let postgres = render_model("Project", false, true, false, &fields, Dialect::Postgres);
        assert_eq!(postgres.matches("database = \"postgres\"").count(), 3);
        let mysql = render_model("Project", true, true, false, &fields, Dialect::Mysql);
        assert_eq!(mysql.matches("database = \"mysql\"").count(), 3);
        assert!(mysql.contains("#[derive(Serialize, Deserialize, JsonSchema, ApiComponent, HttpCreate, SqlxNewModel)]"));
    }

    #[test]
    fn migration_flag_requires_fields() {
        assert!(Opt::from_iter_safe(&["octopux", "generate-model", "--name", "Project", "--migration"]).is_err());
        assert!(Opt::from_iter_safe(&["octopux", "generate-model", "--name", "Project", "--migration", "--fields"]).is_ok());
    }

    #[test]
    fn migration_creates_the_model_table() {
        let fields = vec![
            field("title", "String"),
            field("stars", "i32"),
            field("views", "i64"),
            field("score", "f64"),
            field("done", "bool"),
            field("summary", "Option<String>"),
            field("cover", "Option<Vec<u8>>"),
            field("hits", "u64"),
        ];
        assert_eq!(render_migration("Project", &fields, false, Dialect::Sqlite).unwrap(), "CREATE TABLE IF NOT EXISTS project (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    stars INTEGER NOT NULL,
    views INTEGER NOT NULL,
    score REAL NOT NULL,
    done BOOLEAN NOT NULL,
    summary TEXT,
    cover BLOB,
    hits INTEGER NOT NULL
);
");
        assert_eq!(render_migration("Project", &[], false, Dialect::Sqlite).unwrap(), "CREATE TABLE IF NOT EXISTS project (\n    id INTEGER PRIMARY KEY AUTOINCREMENT\n);\n");
    }

    #[test]
    fn sources_go_in_the_src_folder_when_it_exists() {
        let root = std::env::temp_dir().join(format!("octopux-source-path-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(source_path(&root, "project.rs"), "project.rs");
        std::fs::create_dir_all(root.join("src")).unwrap();
        assert_eq!(source_path(&root, "project.rs"), "src/project.rs");
        assert_eq!(source_path(&root.join("src"), "project.rs"), "project.rs");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn migrations_go_next_to_the_src_folder() {
        use std::path::{Path, PathBuf};
        assert_eq!(migrations_dir(Path::new("/app")), PathBuf::from("migrations"));
        assert_eq!(migrations_dir(Path::new("/app/src")), PathBuf::from("../migrations"));
        assert_eq!(migrations_dir(Path::new("/app/src/models")), PathBuf::from("../../migrations"));
    }

    #[test]
    fn migration_rejects_types_without_column_type() {
        let fields = vec![
            field("tags", "Vec<String>"),
            field("title", "String"),
            field("meta", "Option<serde_json::Value>"),
        ];
        assert_eq!(
            render_migration("Project", &fields, false, Dialect::Sqlite),
            Err("no SQLite column type for tags: Vec<String>, meta: Option<serde_json::Value>".to_string())
        );
    }

    #[test]
    fn read_fields_asks_again_for_a_type_without_column_type() {
        let mut output = Vec::new();
        let input = "tags\nVec<String>\nchrono::NaiveDate\n\n";
        let fields = read_fields(&mut input.as_bytes(), &mut output, false, Some(Dialect::Sqlite), None, false).unwrap();
        assert_eq!(fields, vec![field("tags", "chrono::NaiveDate")]);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("`Vec<String>` has no SQLite column type, use one of String, i8"));
        // without a migration any type is accepted
        let fields = read_fields(&mut "tags\nVec<String>\n\n".as_bytes(), &mut Vec::new(), false, None, None, false).unwrap();
        assert_eq!(fields, vec![field("tags", "Vec<String>")]);
    }

    #[test]
    fn timestamps_are_added_to_the_model_and_updatable_structs() {
        let fields = vec![field("title", "String")];
        let model = render_model("Project", false, false, true, &fields, Dialect::Sqlite);
        assert!(model.contains("use chrono::{DateTime, Utc};"));
        assert!(model.contains("pub struct Project {\n        pub id: Id,\n        pub title: String,\n        pub created_at: Option<DateTime<Utc>>,\n        pub updated_at: Option<DateTime<Utc>>,\n        pub deleted_at: Option<DateTime<Utc>>,\n    }"));
        assert!(model.contains("pub struct NewProject {\n        pub title: String,\n    }"));
        assert!(model.contains("pub struct UpdatableProject {\n        pub id: Id,\n        pub title: String,\n        pub updated_at: Option<DateTime<Utc>>,\n    }"));
        assert!(!render_model("Project", false, false, false, &fields, Dialect::Sqlite).contains("chrono"));
    }

    #[test]
    fn migration_adds_timestamp_columns() {
        let fields = vec![field("title", "String")];
        assert_eq!(
            render_migration("Project", &fields, true, Dialect::Sqlite).unwrap(),
            "CREATE TABLE IF NOT EXISTS project (\n    id INTEGER PRIMARY KEY AUTOINCREMENT,\n    title TEXT NOT NULL,\n    created_at DATETIME,\n    updated_at DATETIME,\n    deleted_at DATETIME\n);\n"
        );
    }

    #[test]
    fn timestamps_reserve_their_field_names() {
        let mut output = Vec::new();
        let fields = read_fields(&mut "created_at\nupdated_at\ndeleted_at\ntitle\n\n".as_bytes(), &mut output, true, None, None, false).unwrap();
        assert_eq!(fields, vec![field("title", "String")]);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Field `created_at` is already declared"));
        assert!(output.contains("Field `updated_at` is already declared"));
        assert!(output.contains("Field `deleted_at` is already declared"));
    }

    #[test]
    fn migration_timestamp_is_utc_date_and_time() {
        assert_eq!(migration_timestamp(0), "19700101000000");
        assert_eq!(migration_timestamp(951782400), "20000229000000");
        assert_eq!(migration_timestamp(1790595045), "20260928113045");
    }

    #[test]
    fn date_fields_import_chrono_and_get_date_columns() {
        let fields = vec![
            field("published_at", "DateTime<Utc>"),
            field("release", "NaiveDate"),
            field("seen_at", "Option<NaiveDateTime>"),
            field("opens", "NaiveTime"),
        ];
        let model = render_model("Project", false, false, false, &fields, Dialect::Sqlite);
        assert!(model.contains("use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};"));
        let model = render_model("Project", false, false, true, &fields[1..2], Dialect::Sqlite);
        assert!(model.contains("use chrono::{DateTime, NaiveDate, Utc};"));
        // NaiveDateTime does not import DateTime
        let model = render_model("Project", false, false, false, &fields[2..3], Dialect::Sqlite);
        assert!(model.contains("use chrono::{NaiveDateTime};"));
        assert!(render_migration("Project", &fields, false, Dialect::Sqlite).unwrap().contains(
            "    published_at DATETIME NOT NULL,\n    release DATE NOT NULL,\n    seen_at DATETIME,\n    opens TIME NOT NULL\n"
        ));
    }

    #[test]
    fn database_flags_conflict() {
        let parse = |flags: &[&str]| {
            let mut args = vec!["octopux", "generate-model", "--name", "Project"];
            args.extend(flags);
            Opt::from_iter_safe(&args)
        };
        assert!(parse(&["--postgres"]).is_ok());
        assert!(parse(&["--postgres", "--mysql"]).is_err());
        assert!(parse(&["--sqlite", "--postgres"]).is_err());
        assert!(parse(&["--sqlite", "--mysql"]).is_err());
        assert_eq!(Dialect::from_flags(false, false), Dialect::Sqlite);
        assert_eq!(Dialect::from_flags(true, false), Dialect::Postgres);
        assert_eq!(Dialect::from_flags(false, true), Dialect::Mysql);
    }

    #[test]
    fn postgres_migration_uses_postgres_types() {
        let fields = vec![
            field("title", "String"),
            field("stars", "i32"),
            field("views", "i64"),
            field("score", "f64"),
            field("done", "bool"),
            field("tags", "Vec<String>"),
            field("cover", "Option<Vec<u8>>"),
            field("at", "DateTime<Utc>"),
            field("ratios", "Vec<f32>"),
            field("seen", "Vec<DateTime<Utc>>"),
            field("days", "Option<Vec<NaiveDate>>"),
            field("logs", "Vec<NaiveDateTime>"),
            field("slots", "Vec<NaiveTime>"),
            field("blobs", "Vec<Vec<u8>>"),
        ];
        assert_eq!(render_migration("Project", &fields, true, Dialect::Postgres).unwrap(), "CREATE TABLE IF NOT EXISTS project (
    id BIGSERIAL PRIMARY KEY,
    title VARCHAR(255) NOT NULL,
    stars INT4 NOT NULL,
    views INT8 NOT NULL,
    score FLOAT8 NOT NULL,
    done BOOL NOT NULL,
    tags TEXT[] NOT NULL,
    cover BYTEA,
    at TIMESTAMPTZ NOT NULL,
    ratios FLOAT4[] NOT NULL,
    seen TIMESTAMPTZ[] NOT NULL,
    days DATE[],
    logs TIMESTAMP[] NOT NULL,
    slots TIME[] NOT NULL,
    blobs BYTEA[] NOT NULL,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);
");
        let unsigned = vec![field("count", "u32")];
        assert_eq!(
            render_migration("Project", &unsigned, false, Dialect::Postgres),
            Err("no PostgreSQL column type for count: u32".to_string())
        );
    }

    #[test]
    fn mysql_migration_uses_mysql_types() {
        let fields = vec![
            field("title", "String"),
            field("stars", "i32"),
            field("count", "u32"),
            field("score", "f64"),
            field("done", "bool"),
            field("summary", "Option<String>"),
        ];
        assert_eq!(render_migration("Project", &fields, true, Dialect::Mysql).unwrap(), "CREATE TABLE IF NOT EXISTS project (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    title VARCHAR(255) NOT NULL,
    stars INT NOT NULL,
    count INT UNSIGNED NOT NULL,
    score DOUBLE NOT NULL,
    done BOOLEAN NOT NULL,
    summary VARCHAR(255),
    created_at DATETIME(6),
    updated_at DATETIME(6),
    deleted_at DATETIME(6)
);
");
        let tags = vec![field("tags", "Vec<String>")];
        assert!(render_migration("Project", &tags, false, Dialect::Mysql).is_err());
    }

    #[test]
    fn read_fields_checks_types_against_the_database() {
        let mut output = Vec::new();
        let fields = read_fields(&mut "tags\nVec<String>\n\n".as_bytes(), &mut output, false, Some(Dialect::Postgres), None, false).unwrap();
        assert_eq!(fields, vec![field("tags", "Vec<String>")]);
        let mut output = Vec::new();
        let fields = read_fields(&mut "count\nu32\ni64\n\n".as_bytes(), &mut output, false, Some(Dialect::Postgres), None, false).unwrap();
        assert_eq!(fields, vec![field("count", "i64")]);
        assert!(String::from_utf8(output).unwrap().contains("`u32` has no PostgreSQL column type, use one of String, i16"));
    }

    fn relation(parent: &str, child: &str, through: Option<&str>) -> Relation {
        Relation::new(parent.into(), child.into(), None, None, through.map(String::from), None)
    }

    #[test]
    fn relation_names_default_to_the_plural_of_the_child() {
        assert_eq!(pluralize("book"), "books");
        assert_eq!(pluralize("category"), "categories");
        assert_eq!(pluralize("day"), "days");
        assert_eq!(pluralize("box"), "boxes");
        assert_eq!(pluralize("branch"), "branches");
        assert_eq!(to_camel_case("favorite_books"), "FavoriteBooks");
        let books = relation("Project", "Book", None);
        assert_eq!(books.name, "books");
        assert_eq!(books.foreign_key, "project_id");
        assert_eq!(books.module(), "project_books");
        assert_eq!(books.type_name(), "ProjectBooks");
        let categories = relation("Project", "ProjectCategory", None);
        assert_eq!(categories.name, "project_categories");
        assert_eq!(categories.type_name(), "ProjectProjectCategories");
        let through = relation("Project", "Category", Some("ProjectCategory"));
        assert_eq!(through.through, Some(Through { model: "ProjectCategory".into(), child_key: "category_id".into() }));
        let named = Relation::new("Project".into(), "Book".into(), Some("drafts".into()), Some("owner_id".into()), None, None);
        assert_eq!((named.name.as_str(), named.foreign_key.as_str(), named.module()), ("drafts", "owner_id", "project_drafts".to_string()));
    }

    #[test]
    fn relation_implements_has_many_on_its_own_type() {
        let rel = render_relation(&relation("Project", "Book", None), false, false, false, Dialect::Sqlite);
        assert!(rel.contains("use crate::project::{Project, Id};\n    use crate::book::Book;"));
        assert!(rel.contains("pub struct ProjectBooksQuery {\n        /// Number of rows to skip\n        pub offset: Option<usize>,"));
        assert!(rel.contains("pub struct ProjectBooks;"));
        assert!(rel.contains("impl HasMany for ProjectBooks {\n        type Parent = Project;\n        type Id = Id;\n        type Query = ProjectBooksQuery;\n        type Result = Vec<Book>;\n        type State = AppState;\n        const RELATION: &'static str = \"books\";"));
        assert!(rel.contains("async fn list_related(id: Id, _query: &ProjectBooksQuery, _state: &AppState) -> Result<Option<Vec<Book>>> {"));
        assert!(rel.contains("pub fn configure(cfg: &mut actix_web::web::ServiceConfig) {\n        gen_relation_endpoint!(ProjectBooks)(cfg)\n    }"));
        assert!(rel.contains("`.configure(project_books::configure)`"));
        assert!(!rel.contains("JsonSchema"));
        assert!(!rel.contains("sqlx"));
    }

    #[test]
    fn openapi_relation_is_documented() {
        let rel = render_relation(&relation("Project", "Book", None), true, false, false, Dialect::Sqlite);
        assert!(rel.contains("#[derive(Deserialize, JsonSchema, ApiComponent)]\n    pub struct ProjectBooksQuery"));
        assert!(rel.contains("pub fn configure(cfg: &mut apistos::web::ServiceConfig) {\n        gen_documented_relation_endpoint!(ProjectBooks)(cfg)\n    }"));
    }

    #[test]
    fn sqlx_relation_pages_the_children_and_looks_up_the_parent() {
        let rel = render_relation(&relation("Project", "Book", None), false, true, false, Dialect::Sqlite);
        assert!(rel.contains("const DEFAULT_LIMIT: i64 = 20;"));
        assert!(rel.contains("async fn list_related(id: Id, query: &ProjectBooksQuery, state: &AppState)"));
        assert!(rel.contains("let models = sqlx::query_as::<_, Book>(\n                \"SELECT * FROM book WHERE project_id = $1 ORDER BY id LIMIT $2 OFFSET $3\",\n            )\n            .bind(id)\n            .bind(limit)\n            .bind(offset)\n            .fetch_all(&state.pool)"));
        assert!(rel.contains("if models.is_empty() {\n                let parent = sqlx::query_scalar::<_, Id>(\n                    \"SELECT id FROM project WHERE id = $1\","));
        assert!(rel.contains("if parent.is_none() {\n                    return Ok(None);\n                }\n            }\n            Ok(Some(models))"));
        let mysql = render_relation(&relation("Project", "Book", None), false, true, true, Dialect::Mysql);
        assert!(mysql.contains("\"SELECT * FROM book WHERE project_id = ? AND deleted_at IS NULL ORDER BY id LIMIT ? OFFSET ?\""));
        assert!(mysql.contains("\"SELECT id FROM project WHERE id = ? AND deleted_at IS NULL\""));
    }

    #[test]
    fn sqlx_relation_joins_the_through_table() {
        let rel = render_relation(&relation("Project", "Category", Some("ProjectCategory")), false, true, false, Dialect::Postgres);
        assert!(rel.contains("\"SELECT category.* FROM category JOIN projectcategory ON projectcategory.category_id = category.id WHERE projectcategory.project_id = $1 ORDER BY category.id LIMIT $2 OFFSET $3\""));
        let rel = render_relation(&relation("Project", "Category", Some("ProjectCategory")), false, true, true, Dialect::Postgres);
        assert!(rel.contains("WHERE projectcategory.project_id = $1 AND category.deleted_at IS NULL AND projectcategory.deleted_at IS NULL ORDER BY"));
    }

    #[test]
    fn relation_migration_indexes_the_foreign_key() {
        assert_eq!(
            render_relation_migration(&relation("Project", "Book", None), Dialect::Sqlite),
            "CREATE INDEX IF NOT EXISTS book_project_id_idx ON book (project_id);\n"
        );
        assert_eq!(
            render_relation_migration(&relation("Project", "Category", Some("ProjectCategory")), Dialect::Mysql),
            "CREATE INDEX projectcategory_project_id_idx ON projectcategory (project_id);\n"
        );
    }

    #[test]
    fn child_key_requires_through() {
        let parse = |flags: &[&str]| {
            let mut args = vec!["octopux", "generate-relation", "--parent", "Project", "--child", "Category"];
            args.extend(flags);
            Opt::from_iter_safe(&args)
        };
        assert!(parse(&[]).is_ok());
        assert!(parse(&["--child-key", "cat_id"]).is_err());
        assert!(parse(&["--through", "ProjectCategory", "--child-key", "cat_id"]).is_ok());
        assert!(parse(&["--postgres", "--mysql"]).is_err());
    }

    #[test]
    fn force_is_accepted_by_the_generators() {
        assert!(Opt::from_iter_safe(&["octopux", "generate-model", "--name", "Project", "--force"]).is_ok());
        assert!(Opt::from_iter_safe(&["octopux", "generate-relation", "--parent", "Project", "--child", "Book", "--force"]).is_ok());
    }

    #[test]
    fn bootstrap_is_a_root_flag() {
        let cli = Cli::from_iter_safe(&["octopux", "--bootstrap"]).unwrap();
        assert!(cli.bootstrap && cli.cmd.is_none());
        let cli = Cli::from_iter_safe(&["octopux", "generate-model", "--name", "Project"]).unwrap();
        assert!(!cli.bootstrap && cli.cmd.is_some());
        assert!(Cli::from_iter_safe(&["octopux", "--bootstrap", "--openapi"]).unwrap().openapi);
        assert!(Cli::from_iter_safe(&["octopux", "--openapi"]).is_err());
    }

    #[test]
    fn bootstrap_writes_main_and_helpers() {
        let root = std::env::temp_dir().join(format!("octopux-bootstrap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        bootstrap(&root, false, &mut "".as_bytes(), &mut Vec::new()).unwrap();
        let main = std::fs::read_to_string(root.join("src/main.rs")).unwrap();
        let helpers = std::fs::read_to_string(root.join("src/helpers.rs")).unwrap();
        assert!(main.starts_with(&(generated_header("//") + "mod helpers;\nuse ")));
        assert!(!main.contains("mod project;"));
        assert!(main.contains(" web::scope(\"v1\"),"));
        assert!(!main.contains("apistos"));
        assert!(!main.contains(".configure(project::configure)"));
        assert!(helpers.contains("pub pool: SqlitePool,"));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn openapi_bootstrap_writes_an_apistos_main() {
        let root = std::env::temp_dir().join(format!("octopux-bootstrap-openapi-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        bootstrap(&root, true, &mut "".as_bytes(), &mut Vec::new()).unwrap();
        let main = std::fs::read_to_string(root.join("src/main.rs")).unwrap();
        let helpers = std::fs::read_to_string(root.join("src/helpers.rs")).unwrap();
        assert!(main.starts_with(&(generated_header("//") + "mod helpers;\nuse ")));
        assert!(!main.contains("mod project;"));
        assert!(main.contains("apistos::web::scope(\"v1\"),"));
        assert!(!main.contains(".configure(project::configure)"));
        assert!(helpers.contains("pub pool: SqlitePool,"));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn bootstrap_overwrites_existing_main_once_confirmed() {
        let root = std::env::temp_dir().join(format!("octopux-bootstrap-overwrite-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();
        let mut output = Vec::new();
        bootstrap(&root, false, &mut "y\n".as_bytes(), &mut output).unwrap();
        assert!(String::from_utf8(output).unwrap().contains("main.rs already exists, overwrite it with the generated one? (y/N) "));
        assert!(std::fs::read_to_string(root.join("src/main.rs")).unwrap().starts_with(&(generated_header("//") + "mod helpers;\n")));
        assert!(root.join("src/helpers.rs").exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn generated_header_comments_the_logo_and_notice() {
        let header = generated_header("--");
        assert!(header.trim_end().lines().all(|line| line.starts_with("--")));
        assert!(header.contains("-- Don't touch this file.\n-- It is automatically generated by the octopux crate cli.\n"));
        assert!(header.ends_with("cli.\n\n"));
        assert_eq!(header.lines().count(), LOGO.trim_matches('\n').lines().count() + 4);
    }

    #[test]
    fn confirm_defaults_to_no() {
        let answer = |input: &str| confirm(&mut input.as_bytes(), &mut Vec::new(), "Install?").unwrap();
        assert!(answer("y\n"));
        assert!(answer("YES\n"));
        assert!(!answer("\n"));
        assert!(!answer("n\n"));
        assert!(!answer("maybe\n"));
        assert!(!answer(""));
    }

    #[test]
    fn confirm_save_defaults_to_yes() {
        let files = [("project.rs".to_string(), false)];
        let answer = |input: &str| confirm_save(&mut input.as_bytes(), &mut Vec::new(), &files).unwrap();
        assert!(answer("\n"));
        assert!(answer("y\n"));
        assert!(answer(""));
        assert!(!answer("n\n"));
        assert!(!answer("NO\n"));
    }

    #[test]
    fn changes_summary_lists_the_created_and_overwritten_files() {
        let summary = changes_summary(&[("project.rs".to_string(), true), ("migrations/1_create_project.sql".to_string(), false)]);
        assert!(summary.starts_with("2 files to write"));
        assert!(summary.contains("  ~ project.rs (overwritten)"));
        assert!(summary.contains("  + migrations/1_create_project.sql"));
    }

    #[test]
    fn bootstrap_dependencies_pin_octopux_to_the_cli_version() {
        let tag = format!("v{}", env!("CARGO_PKG_VERSION"));
        let deps = bootstrap_dependencies(true);
        assert_eq!(deps[0], ["octopux", "--git", "https://github.com/ctaque/octopux", "--tag", tag.as_str(), "--features", "openapi,sqlx"]);
        assert!(deps.iter().any(|d| d == &["apistos-schemars@0.8", "--rename", "schemars"]));
        let deps = bootstrap_dependencies(false);
        assert_eq!(deps[0], ["octopux", "--git", "https://github.com/ctaque/octopux", "--tag", tag.as_str(), "--features", "sqlx"]);
        assert!(!deps.iter().any(|d| d[0].starts_with("apistos")));
    }
}
