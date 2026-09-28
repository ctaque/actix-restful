use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use sqlx::TypeInfo;
use structopt::StructOpt;
use std::fs::{self, File};
use std::io::{self, BufRead, Write, Error};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, StructOpt)]
#[structopt(name = "actix-restful")]
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
        /// Fills the model functions with sqlx queries on the `pool` of the AppState, requires --fields
        #[structopt(long = "sqlx", requires = "fields")]
        sqlx: bool,
        /// Creates the migration of the model table in the migrations folder next to src, requires --fields
        #[structopt(long = "migration", requires = "fields")]
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
        /// Adds `created_at`, `updated_at` and `deleted_at` columns to the model and the migration,
        /// set by the sqlx queries, `delete` becomes a soft delete setting `deleted_at`
        #[structopt(long = "timestamps")]
        timestamps: bool,
    }
}

const OPENAPI_IMPORTS: &str = r#"
    use apistos::ApiComponent;
    use schemars::JsonSchema;
    use actix_restful::gen_documented_endpoint;"#;

const OPENAPI_CONFIGURE: &str = r#"

    // Registers the documented routes of the {entity_lower_case} endpoint
    // (actix-restful `openapi` feature), to mount with `.configure({entity_lower_case}::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!({entity}, New{entity}, Updatable{entity})(cfg)
    }
"#;

const OPENAPI_DERIVES: &str = ", JsonSchema, ApiComponent";

// Columns added by --timestamps, all nullable, `updated_at` is also part of the updatable struct
const TIMESTAMP_COLUMNS: [&str; 3] = ["created_at", "updated_at", "deleted_at"];
const TIMESTAMP_TYPE: &str = "Option<DateTime<Utc>>";

#[derive(Debug, PartialEq)]
struct Field {
    name: String,
    ty: String,
}

// Types proposed when prompting for a field type, the first one is the default
const FIELD_TYPES: &[&str] = &[
    "String", "i32", "i64", "f64", "bool", "Option<String>", "DateTime<Utc>", "NaiveDateTime", "NaiveDate", "NaiveTime", "Vec<u8>",
];

fn field_types_menu() -> String {
    FIELD_TYPES
        .iter()
        .enumerate()
        .map(|(i, ty)| format!("{}) {}", i + 1, ty))
        .collect::<Vec<_>>()
        .join("  ")
}

// Resolves a type answer: empty for the default, a number from the menu, or any custom type
fn parse_field_type(answer: &str) -> Option<String> {
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

fn prompt<R: BufRead, W: Write>(input: &mut R, output: &mut W, message: &str) -> Result<Option<String>, Error> {
    write!(output, "{}", message)?;
    output.flush()?;
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    Ok(Some(line.trim().to_string()))
}

// Asks for field names and types until an empty name (or end of input) is entered,
// `timestamps` reserves the `created_at`, `updated_at` and `deleted_at` names,
// with a `dialect`, only accepts types with a column type in its database (see `Dialect::sql_types`)
fn read_fields<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    timestamps: bool,
    dialect: Option<Dialect>,
) -> Result<Vec<Field>, Error> {
    let mut fields: Vec<Field> = Vec::new();
    let reserved: &[&str] = if timestamps { &["id", "created_at", "updated_at", "deleted_at"] } else { &["id"] };
    if timestamps {
        writeln!(output, "Enter the model fields (empty name to finish), `id: Id`, `created_at`, `updated_at` and `deleted_at` are already declared")?;
    } else {
        writeln!(output, "Enter the model fields (empty name to finish), `id: Id` is already declared")?;
    }
    loop {
        let name = match prompt(input, output, "Field name: ")? {
            Some(name) if !name.is_empty() => name,
            _ => break,
        };
        let snake = to_snake_case(&name);
        if !is_field_name(&snake) {
            writeln!(output, "`{}` is not a valid field name, use snake_case", name)?;
            continue;
        }
        if snake != name {
            writeln!(output, "  `{}` renamed to `{}`", name, snake)?;
        }
        let name = snake;
        if reserved.contains(&name.as_str()) || fields.iter().any(|f| f.name == name) {
            writeln!(output, "Field `{}` is already declared", name)?;
            continue;
        }
        writeln!(output, "  {}", field_types_menu())?;
        let ty = loop {
            let message = format!("Type of `{}` (number or custom type) [{}]: ", name, FIELD_TYPES[0]);
            let answer = prompt(input, output, &message)?.unwrap_or_default();
            match (parse_field_type(&answer), dialect) {
                (Some(ty), Some(dialect)) if dialect.sql_column_type(&ty).is_none() => writeln!(
                    output,
                    "`{}` has no {} column type, use one of {}, or Option<T> of them",
                    ty,
                    dialect.name(),
                    dialect.sql_types().iter().map(|(ty, _)| *ty).collect::<Vec<_>>().join(", ")
                )?,
                (Some(ty), _) => break ty,
                (None, _) => writeln!(output, "`{}` is not in the list, pick 1 to {}", answer, FIELD_TYPES.len())?,
            }
        };
        fields.push(Field { name, ty });
    }
    Ok(fields)
}

// Bodies of find, list, delete, save and update, as left to the user without --sqlx
const EMPTY_BODIES: [&str; 5] = [
    "            // fetch from somwhere with id",
    "            // list",
    "            // hard or soft delete",
    "            // persist",
    "            // update in db",
];

// Pagination parameters of the list query, only generated with --sqlx
const LIST_QUERY_FIELDS: &str = r#"
        /// Number of rows to skip
        offset: Option<usize>,
        /// Maximum number of rows to return (20 by default, 100 at most)
        limit: Option<usize>,
    "#;

// Clamps the pagination parameters of the list query before binding them
const LIST_PAGINATION: &str = "            let offset = query.offset.unwrap_or(0) as i64;
            let limit = query.limit.map_or(DEFAULT_LIMIT, |l| (l as i64).min(MAX_LIMIT));";

const LIST_LIMITS: &str = r#"
    /// Number of rows returned by list when no limit is given
    const DEFAULT_LIMIT: i64 = 20;
    /// Maximum number of rows returned by list
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
        String, i8, i16, i32, u8, u16, i64, u32, f32, f64, bool,
        DateTime<Utc>, NaiveDateTime, NaiveDate, NaiveTime, Vec<u8>,
    )
}

// PostgreSQL has no unsigned integers and sqlx maps i8 to "char", Vec<T> are arrays
fn postgres_types() -> Vec<(&'static str, String)> {
    sql_types!(sqlx::Postgres;
        String, i16, i32, i64, f32, f64, bool,
        DateTime<Utc>, NaiveDateTime, NaiveDate, NaiveTime, Vec<u8>,
        Vec<String>, Vec<i16>, Vec<i32>, Vec<i64>, Vec<f64>, Vec<bool>,
    )
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
    let mut types = sql_types!(sqlx::MySql;
        String, i8, i16, i32, i64, u8, u16, u32, u64, f32, f64, bool,
        DateTime<Utc>, NaiveDateTime, NaiveDate, NaiveTime, Vec<u8>,
    );
    for (ty, sql) in types.iter_mut() {
        if let Some((_, over)) = MYSQL_OVERRIDES.iter().find(|(t, _)| t == ty) {
            *sql = over.to_string();
        }
    }
    types
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

    // `$1, $2...` for SQLite and PostgreSQL, `?` for MySQL
    fn placeholders(self, count: usize) -> Vec<String> {
        match self {
            Dialect::Mysql => vec!["?".to_string(); count],
            _ => (1..=count).map(|i| format!("${}", i)).collect(),
        }
    }
}

// How a generated query runs
enum Fetch {
    One,
    All,
    Execute,
}

// A query statement of a model function, its result is bound to `var` unless empty
fn sqlx_query(var: &str, result: &str, sql: &str, binds: &[String], fetch: Fetch) -> String {
    let binds: String = binds
        .iter()
        .map(|b| format!("\n            .bind({})", b))
        .collect();
    let (query, method) = match fetch {
        Fetch::One => (format!("sqlx::query_as::<_, {}>", result), "fetch_one"),
        Fetch::All => (format!("sqlx::query_as::<_, {}>", result), "fetch_all"),
        Fetch::Execute => ("sqlx::query".to_string(), "execute"),
    };
    let binding = if var.is_empty() { String::new() } else { format!("let {} = ", var) };
    format!(
        "            {}{}(\n                \"{}\",\n            ){}\n            .{}(&state.pool)\n            .await?;",
        binding, query, sql, binds, method
    )
}

// Statements of a model function followed by its result
fn sqlx_body(statements: &[String], ok: &str) -> String {
    format!("{}\n            Ok({})", statements.join("\n"), ok)
}

// Bodies of find, list, delete, save and update querying the `{entity_lower_case}` table,
// with RETURNING on SQLite and PostgreSQL, and a SELECT of the row on MySQL which has none,
// `timestamps` sets `created_at` and `updated_at` on insert and `updated_at` on update,
// and soft deletes: delete sets `deleted_at`, and find, list and update skip the deleted rows
fn sqlx_bodies(fields: &[Field], timestamps: bool, dialect: Dialect) -> [String; 5] {
    let table = "{entity_lower_case}";
    let mysql = dialect == Dialect::Mysql;
    let columns: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
    let self_binds: Vec<String> = columns.iter().map(|c| format!("self.{}", c)).collect();
    let (mut insert_columns, mut insert_binds) = (columns.clone(), self_binds.clone());
    let (mut update_columns, mut update_binds) = (columns.clone(), self_binds.clone());
    if timestamps {
        insert_columns.extend(["created_at", "updated_at"]);
        insert_binds.extend(["now".to_string(), "now".to_string()]);
        update_columns.push("updated_at");
        update_binds.push("Utc::now()".to_string());
    }
    let mut placeholders = dialect.placeholders(update_columns.len() + 1);
    let id_placeholder = placeholders.pop().unwrap_or_default();
    let assignments: Vec<String> = update_columns
        .iter()
        .zip(placeholders)
        .map(|(c, p)| format!("{} = {}", c, p))
        .collect();
    update_binds.push("self.id".to_string());
    let first = dialect.placeholders(1).join("");
    let not_deleted = if timestamps { " AND deleted_at IS NULL" } else { "" };
    let select_by_id = format!("SELECT * FROM {} WHERE id = {}", table, first);
    let select_live_by_id = format!("{}{}", select_by_id, not_deleted);
    let self_id = ["self.id".to_string()];
    let insert = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        table,
        insert_columns.join(", "),
        dialect.placeholders(insert_columns.len()).join(", ")
    );
    let update = format!(
        "UPDATE {} SET {} WHERE id = {}{}",
        table,
        assignments.join(", "),
        id_placeholder,
        not_deleted
    );
    let (delete, delete_binds) = if timestamps {
        let placeholders = dialect.placeholders(2);
        (
            format!("UPDATE {} SET deleted_at = {} WHERE id = {}{}", table, placeholders[0], placeholders[1], not_deleted),
            vec!["now".to_string(), "self.id".to_string()],
        )
    } else {
        (format!("DELETE FROM {} WHERE id = {}", table, first), self_id.to_vec())
    };
    let updated_columns = format!("id, {}", update_columns.join(", "));
    let (mut delete_statements, mut save_statements, update_body) = if mysql {
        // the soft deleted row is selected before its update, and gets the `deleted_at` it was given
        let (model_var, deleted_at) = if timestamps {
            ("mut model", vec!["            model.deleted_at = Some(now);".to_string()])
        } else {
            ("model", vec![])
        };
        (
            [
                vec![
                    sqlx_query(model_var, "{entity}", &select_live_by_id, &self_id, Fetch::One),
                    sqlx_query("", "", &delete, &delete_binds, Fetch::Execute),
                ],
                deleted_at,
            ]
            .concat(),
            vec![
                sqlx_query("result", "", &insert, &insert_binds, Fetch::Execute),
                sqlx_query(
                    "model",
                    "{entity}",
                    &select_by_id,
                    &["result.last_insert_id() as Id".to_string()],
                    Fetch::One,
                ),
            ],
            sqlx_body(
                &[
                    sqlx_query("", "", &update, &update_binds, Fetch::Execute),
                    sqlx_query(
                        "model",
                        "Updatable{entity}",
                        &format!("SELECT {} FROM {} WHERE id = {}{}", updated_columns, table, first, not_deleted),
                        &self_id,
                        Fetch::One,
                    ),
                ],
                "model",
            ),
        )
    } else {
        (
            vec![sqlx_query("model", "{entity}", &format!("{} RETURNING *", delete), &delete_binds, Fetch::One)],
            vec![sqlx_query("model", "{entity}", &format!("{} RETURNING *", insert), &insert_binds, Fetch::One)],
            sqlx_body(
                &[sqlx_query(
                    "model",
                    "Updatable{entity}",
                    &format!("{} RETURNING {}", update, updated_columns),
                    &update_binds,
                    Fetch::One,
                )],
                "model",
            ),
        )
    };
    // created_at and updated_at get the same value
    if timestamps {
        save_statements.insert(0, "            let now = Utc::now();".to_string());
        delete_statements.insert(0, "            let now = Utc::now();".to_string());
    }
    [
        sqlx_body(
            &[sqlx_query("model", "{entity}", &select_live_by_id, &["id".to_string()], Fetch::One)],
            "Box::new(model)",
        ),
        sqlx_body(
            &[
                LIST_PAGINATION.to_string(),
                sqlx_query(
                    "models",
                    "{entity}",
                    &format!(
                        "SELECT * FROM {}{} ORDER BY id LIMIT {}",
                        table,
                        if timestamps { " WHERE deleted_at IS NULL" } else { "" },
                        dialect.placeholders(2).join(" OFFSET ")
                    ),
                    &["limit".to_string(), "offset".to_string()],
                    Fetch::All,
                ),
            ],
            "models",
        ),
        sqlx_body(&delete_statements, "model"),
        sqlx_body(&save_statements, "model"),
        update_body,
    ]
}

// Fails with the fields whose type has no column type in the `dialect` database
fn render_migration(name: &str, fields: &[Field], timestamps: bool, dialect: Dialect) -> Result<String, String> {
    let mut columns = vec![dialect.id_column().to_string()];
    let mut unmapped = Vec::new();
    for field in fields {
        match dialect.sql_column_type(&field.ty) {
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
    Ok(format!(
        "CREATE TABLE IF NOT EXISTS {} (\n    {}\n);\n",
        name.to_lowercase(),
        columns.join(",\n    ")
    ))
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
        .map(|f| format!("\n        {}: {},", f.name, f.ty))
        .collect()
}

fn render_model(name: &str, openapi: bool, sqlx: bool, timestamps: bool, fields: &[Field], dialect: Dialect) -> String {
    let (imports, derives, configure) = if openapi {
        (OPENAPI_IMPORTS, OPENAPI_DERIVES, OPENAPI_CONFIGURE)
    } else {
        ("", "", "")
    };
    let field_lines = struct_fields(fields);
    // keeps the blank line of the empty creatable struct
    let new_fields = if fields.is_empty() { "\n" } else { &field_lines };
    let chrono_imports = chrono_imports(fields, timestamps);
    let (model_fields, updatable_fields) = if timestamps {
        let timestamp = |name: &str| Field { name: name.to_string(), ty: TIMESTAMP_TYPE.to_string() };
        (
            field_lines.clone() + &struct_fields(&TIMESTAMP_COLUMNS.map(timestamp)),
            field_lines.clone() + &struct_fields(&[timestamp("updated_at")]),
        )
    } else {
        (field_lines.clone(), field_lines.clone())
    };
    let bodies = if sqlx {
        sqlx_bodies(fields, timestamps, dialect)
    } else {
        EMPTY_BODIES.map(String::from)
    };
    let (sqlx_derives, state, mut_self) = if sqlx {
        (", sqlx::FromRow", "state", "")
    } else {
        ("", "_state", "mut ")
    };
    let (list_query_fields, list_query, list_limits) = if sqlx {
        (LIST_QUERY_FIELDS, "query", LIST_LIMITS)
    } else {
        ("", "_query", "")
    };
    let [find_body, list_body, delete_body, save_body, update_body] = bodies;
    MODEL_TPL
        .replace("{find_body}", &find_body)
        .replace("{list_body}", &list_body)
        .replace("{delete_body}", &delete_body)
        .replace("{save_body}", &save_body)
        .replace("{update_body}", &update_body)
        .replace("{sqlx_derives}", sqlx_derives)
        .replace("{state}", state)
        .replace("{mut_self}", mut_self)
        .replace("{list_query_fields}", list_query_fields)
        .replace("{list_query}", list_query)
        .replace("{list_limits}", list_limits)
        .replace("{model_fields}", &model_fields)
        .replace("{updatable_fields}", &updatable_fields)
        .replace("{chrono_imports}", &chrono_imports)
        .replace("{new_fields}", new_fields)
        .replace("{openapi_imports}", imports)
        .replace("{openapi_derives}", derives)
        .replace("{openapi_configure}", configure)
        .replace("{entity}", name)
        .replace("{entity_lower_case}", &name.to_lowercase())
}

const MODEL_TPL: &str = r#"
    use serde::{Serialize, Deserialize};
    use actix_restful::{
        HttpCreate,
        HttpFindListDelete,
        HttpUpdate,
        Model,
        NewModel,
        UpdatableModel,
        RestfulPathInfo
    };
    use actix_restful_derive::{HttpCreate, HttpFindListDelete, HttpUpdate, actix_restful_info};

    use anyhow::Result;
    use async_trait::async_trait;
    use std::default::Default;
    use actix_web;{chrono_imports}{openapi_imports}

    #[derive(Default, Deserialize{openapi_derives})]
    struct FindQuery {}
    #[derive(Deserialize{openapi_derives})]
    struct ListQuery {{list_query_fields}}
    #[derive(Deserialize{openapi_derives})]
    struct DeleteQuery {}
    type ListResult = Vec<{entity}>;
    type DeleteResult = {entity};
    #[derive(Deserialize{openapi_derives})]
    struct SaveQuery {}
    #[derive(Deserialize{openapi_derives})]
    struct UpdateQuery {}
    type Id = i64;{list_limits}

    #[derive(Default, Serialize, Deserialize{openapi_derives}{sqlx_derives}, HttpFindListDelete)]
    #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
    #[actix_restful_info(path = "{entity_lower_case}")]
    struct {entity} {
        id: Id,{model_fields}
    }
    
    #[async_trait]
    impl Model<Id, FindQuery, ListQuery, ListResult, DeleteQuery, DeleteResult, AppState> for {entity} {
        async fn find(id: Id, _query: &FindQuery, {state}: &AppState) -> Result<Box<{entity}>> {
{find_body}
        }
        async fn list({list_query}: &ListQuery, {state}: &AppState) -> Result<ListResult> {
{list_body}
        }
        async fn delete({mut_self}self: Self, _query: &DeleteQuery, {state}: &AppState) -> Result<DeleteResult> {
{delete_body}
        }
    }
    
    #[derive(Serialize, Deserialize{openapi_derives}, HttpCreate)]
    #[http_create(SaveQuery, AppState)]
    struct New{entity} {{new_fields}
    }
    #[async_trait]
    impl NewModel<{entity}, SaveQuery, AppState> for New{entity} {
        async fn save(self: Self, _query: &SaveQuery, {state}: &AppState) -> Result<{entity}> {
{save_body}
        }
    }
    
    #[derive(Serialize, Deserialize{openapi_derives}{sqlx_derives}, HttpUpdate)]
    #[http_update(Id, UpdateQuery, {entity}, FindQuery, AppState)]
    struct Updatable{entity} {
        id: Id,{updatable_fields}
    }
    #[async_trait]
    impl UpdatableModel<Updatable{entity}, UpdateQuery, AppState> for Updatable{entity} {
        async fn update({mut_self}self: Self, _query: &UpdateQuery, {state}: &AppState) -> Result<Updatable{entity}> {
{update_body}
        }
    }{openapi_configure}
    "#;

fn main() -> Result<(), Error> {
    let opt = Opt::from_args();

    match opt {
        Opt::GenerateModel { name, openapi, fields, sqlx, migration, sqlite: _, postgres, mysql, timestamps } => {
            let dialect = Dialect::from_flags(postgres, mysql);
            let fields = if fields {
                let strict = if migration { Some(dialect) } else { None };
                read_fields(&mut io::stdin().lock(), &mut io::stdout(), timestamps, strict)?
            } else {
                Vec::new()
            };
            // the INSERT and UPDATE queries need at least one column
            if sqlx && fields.is_empty() {
                eprintln!("--sqlx needs at least one field, model {} not generated", name);
                process::exit(1);
            }
            let to_write = render_model(&name, openapi, sqlx, timestamps, &fields, dialect);
            let mut path = String::from("");
            path.push_str(&name);
            path.push_str(".rs");
            let mut output = File::create(path.clone())?;
            write!(output, "{}", to_write)?;
            println!("Successfully generated model {}", path);
            if migration {
                let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
                let dir = migrations_dir(&std::env::current_dir()?);
                fs::create_dir_all(&dir)?;
                let path = dir
                    .join(format!("{}_create_{}.sql", migration_timestamp(secs), name.to_lowercase()))
                    .display()
                    .to_string();
                let sql = render_migration(&name, &fields, timestamps, dialect).unwrap_or_else(|e| {
                    eprintln!("{}, migration not generated", e);
                    process::exit(1);
                });
                fs::write(&path, sql)?;
                println!("Successfully generated migration {}", path);
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{migration_timestamp, migrations_dir, parse_field_type, read_fields, render_migration, render_model, to_snake_case, Dialect, Field, Opt};
    use structopt::StructOpt;

    #[test]
    fn default_model_has_no_openapi_derives() {
        let model = render_model("Project", false, false, false, &[], Dialect::Sqlite);
        assert!(!model.contains("JsonSchema"));
        assert!(!model.contains("ApiComponent"));
        assert!(!model.contains("fn configure"));
        assert!(!model.contains("{openapi"));
        assert!(model.contains("#[actix_restful_info(path = \"project\")]"));
    }

    #[test]
    fn openapi_model_derives_schemas_on_every_route_type() {
        let model = render_model("Project", true, false, false, &[], Dialect::Sqlite);
        assert!(model.contains("use apistos::ApiComponent;"));
        assert!(model.contains("use schemars::JsonSchema;"));
        assert!(model.contains("use actix_restful::gen_documented_endpoint;"));
        assert!(model.contains("pub fn configure(cfg: &mut apistos::web::ServiceConfig) {"));
        assert!(model.contains("gen_documented_endpoint!(Project, NewProject, UpdatableProject)(cfg)"));
        // 5 query structs + Project, NewProject and UpdatableProject
        assert_eq!(model.matches(", JsonSchema, ApiComponent").count(), 8);
        assert!(!model.contains("{openapi"));
        assert!(!model.contains("{entity"));
    }

    #[test]
    fn model_without_fields_keeps_empty_structs() {
        let model = render_model("Project", false, false, false, &[], Dialect::Sqlite);
        assert!(model.contains("struct Project {\n        id: Id,\n    }"));
        assert!(model.contains("struct NewProject {\n\n    }"));
        assert!(model.contains("struct UpdatableProject {\n        id: Id,\n    }"));
        assert!(!model.contains("_fields}"));
    }

    #[test]
    fn fields_are_added_to_every_model_struct() {
        let fields = vec![
            Field { name: "title".into(), ty: "String".into() },
            Field { name: "stars".into(), ty: "i32".into() },
        ];
        let model = render_model("Project", false, false, false, &fields, Dialect::Sqlite);
        assert!(model.contains("struct Project {\n        id: Id,\n        title: String,\n        stars: i32,\n    }"));
        assert!(model.contains("struct NewProject {\n        title: String,\n        stars: i32,\n    }"));
        assert!(model.contains("struct UpdatableProject {\n        id: Id,\n        title: String,\n        stars: i32,\n    }"));
    }

    #[test]
    fn read_fields_prompts_until_empty_name() {
        // invalid names, default type, duplicate and reserved `id` are handled
        let input = "title\n\n1bad\nstars\ni32\ntitle\nid\n\n";
        let mut output = Vec::new();
        let fields = read_fields(&mut input.as_bytes(), &mut output, false, None).unwrap();
        assert_eq!(fields, vec![
            Field { name: "title".into(), ty: "String".into() },
            Field { name: "stars".into(), ty: "i32".into() },
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
        let fields = read_fields(&mut input.as_bytes(), &mut output, false, None).unwrap();
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
        let fields = read_fields(&mut "title\nString".as_bytes(), &mut output, false, None).unwrap();
        assert_eq!(fields, vec![Field { name: "title".into(), ty: "String".into() }]);
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
        let fields = read_fields(&mut "done\n42\n5\n\n".as_bytes(), &mut output, false, None).unwrap();
        assert_eq!(fields, vec![Field { name: "done".into(), ty: "bool".into() }]);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("1) String  2) i32"));
        assert!(output.contains("`42` is not in the list, pick 1 to 11"));
    }

    #[test]
    fn sqlx_flag_requires_fields() {
        assert!(Opt::from_iter_safe(&["actix-restful", "generate-model", "--name", "Project", "--sqlx"]).is_err());
        assert!(Opt::from_iter_safe(&["actix-restful", "generate-model", "--name", "Project", "--sqlx", "--fields"]).is_ok());
    }

    #[test]
    fn sqlx_model_fills_every_function() {
        let fields = vec![
            Field { name: "title".into(), ty: "String".into() },
            Field { name: "stars".into(), ty: "i32".into() },
        ];
        let model = render_model("Project", false, true, false, &fields, Dialect::Sqlite);
        assert!(!super::EMPTY_BODIES.iter().any(|body| model.contains(body)));
        assert!(!model.contains("mut self"));
        assert!(!model.contains("_state"));
        assert!(model.contains("\"SELECT * FROM project WHERE id = $1\""));
        assert!(model.contains("\"SELECT * FROM project ORDER BY id LIMIT $1 OFFSET $2\""));
        assert!(model.contains("\"DELETE FROM project WHERE id = $1 RETURNING *\""));
        assert!(model.contains("\"INSERT INTO project (title, stars) VALUES ($1, $2) RETURNING *\""));
        assert!(model.contains("\"UPDATE project SET title = $1, stars = $2 WHERE id = $3 RETURNING id, title, stars\""));
        assert!(model.contains("sqlx::query_as::<_, UpdatableProject>("));
        assert!(model.contains(".bind(self.title)\n            .bind(self.stars)\n            .bind(self.id)"));
        assert!(model.contains("Ok(Box::new(model))"));
        assert_eq!(model.matches(", sqlx::FromRow").count(), 2);
    }

    #[test]
    fn sqlx_model_paginates_the_list() {
        let fields = vec![Field { name: "title".into(), ty: "String".into() }];
        let model = render_model("Project", false, true, false, &fields, Dialect::Sqlite);
        assert!(model.contains("struct ListQuery {\n        /// Number of rows to skip\n        offset: Option<usize>,"));
        assert!(model.contains("        limit: Option<usize>,\n    }"));
        assert!(model.contains("const DEFAULT_LIMIT: i64 = 20;"));
        assert!(model.contains("const MAX_LIMIT: i64 = 100;"));
        assert!(model.contains("async fn list(query: &ListQuery, state: &AppState)"));
        assert!(model.contains("let limit = query.limit.map_or(DEFAULT_LIMIT, |l| (l as i64).min(MAX_LIMIT));"));
        assert!(model.contains("ORDER BY id LIMIT $1 OFFSET $2\",\n            )\n            .bind(limit)\n            .bind(offset)\n            .fetch_all(&state.pool)"));
        let mysql = render_model("Project", false, true, false, &fields, Dialect::Mysql);
        assert!(mysql.contains("\"SELECT * FROM project ORDER BY id LIMIT ? OFFSET ?\""));
        // without --sqlx the list query stays empty
        let model = render_model("Project", false, false, false, &fields, Dialect::Sqlite);
        assert!(model.contains("struct ListQuery {}"));
        assert!(model.contains("async fn list(_query: &ListQuery, _state: &AppState)"));
        assert!(!model.contains("LIMIT"));
    }

    #[test]
    fn migration_flag_requires_fields() {
        assert!(Opt::from_iter_safe(&["actix-restful", "generate-model", "--name", "Project", "--migration"]).is_err());
        assert!(Opt::from_iter_safe(&["actix-restful", "generate-model", "--name", "Project", "--migration", "--fields"]).is_ok());
    }

    #[test]
    fn migration_creates_the_model_table() {
        let fields = vec![
            Field { name: "title".into(), ty: "String".into() },
            Field { name: "stars".into(), ty: "i32".into() },
            Field { name: "views".into(), ty: "i64".into() },
            Field { name: "score".into(), ty: "f64".into() },
            Field { name: "done".into(), ty: "bool".into() },
            Field { name: "summary".into(), ty: "Option<String>".into() },
            Field { name: "cover".into(), ty: "Option<Vec<u8>>".into() },
        ];
        assert_eq!(render_migration("Project", &fields, false, Dialect::Sqlite).unwrap(), "CREATE TABLE IF NOT EXISTS project (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    stars INTEGER NOT NULL,
    views INTEGER NOT NULL,
    score REAL NOT NULL,
    done BOOLEAN NOT NULL,
    summary TEXT,
    cover BLOB
);
");
        assert_eq!(render_migration("Project", &[], false, Dialect::Sqlite).unwrap(), "CREATE TABLE IF NOT EXISTS project (\n    id INTEGER PRIMARY KEY AUTOINCREMENT\n);\n");
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
            Field { name: "tags".into(), ty: "Vec<String>".into() },
            Field { name: "title".into(), ty: "String".into() },
            Field { name: "meta".into(), ty: "Option<serde_json::Value>".into() },
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
        let fields = read_fields(&mut input.as_bytes(), &mut output, false, Some(Dialect::Sqlite)).unwrap();
        assert_eq!(fields, vec![Field { name: "tags".into(), ty: "chrono::NaiveDate".into() }]);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("`Vec<String>` has no SQLite column type, use one of String, i8"));
        // without a migration any type is accepted
        let fields = read_fields(&mut "tags\nVec<String>\n\n".as_bytes(), &mut Vec::new(), false, None).unwrap();
        assert_eq!(fields, vec![Field { name: "tags".into(), ty: "Vec<String>".into() }]);
    }

    #[test]
    fn timestamps_are_added_to_the_model_and_updatable_structs() {
        let fields = vec![Field { name: "title".into(), ty: "String".into() }];
        let model = render_model("Project", false, false, true, &fields, Dialect::Sqlite);
        assert!(model.contains("use chrono::{DateTime, Utc};"));
        assert!(model.contains("struct Project {\n        id: Id,\n        title: String,\n        created_at: Option<DateTime<Utc>>,\n        updated_at: Option<DateTime<Utc>>,\n        deleted_at: Option<DateTime<Utc>>,\n    }"));
        assert!(model.contains("struct NewProject {\n        title: String,\n    }"));
        assert!(model.contains("struct UpdatableProject {\n        id: Id,\n        title: String,\n        updated_at: Option<DateTime<Utc>>,\n    }"));
        assert!(!render_model("Project", false, false, false, &fields, Dialect::Sqlite).contains("chrono"));
    }

    #[test]
    fn sqlx_model_sets_timestamps() {
        let fields = vec![Field { name: "title".into(), ty: "String".into() }];
        let model = render_model("Project", false, true, true, &fields, Dialect::Sqlite);
        assert!(model.contains("let now = Utc::now();\n            let model = sqlx::query_as::<_, Project>("));
        assert!(model.contains("\"INSERT INTO project (title, created_at, updated_at) VALUES ($1, $2, $3) RETURNING *\""));
        assert!(model.contains(".bind(self.title)\n            .bind(now)\n            .bind(now)"));
        assert!(model.contains("\"UPDATE project SET title = $1, updated_at = $2 WHERE id = $3 AND deleted_at IS NULL RETURNING id, title, updated_at\""));
        assert!(model.contains(".bind(self.title)\n            .bind(Utc::now())\n            .bind(self.id)"));
        // soft delete, the deleted rows are skipped
        assert!(model.contains("let now = Utc::now();\n            let model = sqlx::query_as::<_, Project>(\n                \"UPDATE project SET deleted_at = $1 WHERE id = $2 AND deleted_at IS NULL RETURNING *\",\n            )\n            .bind(now)\n            .bind(self.id)"));
        assert!(!model.contains("DELETE FROM"));
        assert!(model.contains("\"SELECT * FROM project WHERE id = $1 AND deleted_at IS NULL\""));
        assert!(model.contains("\"SELECT * FROM project WHERE deleted_at IS NULL ORDER BY id LIMIT $1 OFFSET $2\""));
    }

    #[test]
    fn migration_adds_timestamp_columns() {
        let fields = vec![Field { name: "title".into(), ty: "String".into() }];
        assert_eq!(
            render_migration("Project", &fields, true, Dialect::Sqlite).unwrap(),
            "CREATE TABLE IF NOT EXISTS project (\n    id INTEGER PRIMARY KEY AUTOINCREMENT,\n    title TEXT NOT NULL,\n    created_at DATETIME,\n    updated_at DATETIME,\n    deleted_at DATETIME\n);\n"
        );
    }

    #[test]
    fn timestamps_reserve_their_field_names() {
        let mut output = Vec::new();
        let fields = read_fields(&mut "created_at\nupdated_at\ndeleted_at\ntitle\n\n".as_bytes(), &mut output, true, None).unwrap();
        assert_eq!(fields, vec![Field { name: "title".into(), ty: "String".into() }]);
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
            Field { name: "published_at".into(), ty: "DateTime<Utc>".into() },
            Field { name: "release".into(), ty: "NaiveDate".into() },
            Field { name: "seen_at".into(), ty: "Option<NaiveDateTime>".into() },
            Field { name: "opens".into(), ty: "NaiveTime".into() },
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
            let mut args = vec!["actix-restful", "generate-model", "--name", "Project"];
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
            Field { name: "title".into(), ty: "String".into() },
            Field { name: "stars".into(), ty: "i32".into() },
            Field { name: "views".into(), ty: "i64".into() },
            Field { name: "score".into(), ty: "f64".into() },
            Field { name: "done".into(), ty: "bool".into() },
            Field { name: "tags".into(), ty: "Vec<String>".into() },
            Field { name: "cover".into(), ty: "Option<Vec<u8>>".into() },
            Field { name: "at".into(), ty: "DateTime<Utc>".into() },
        ];
        assert_eq!(render_migration("Project", &fields, true, Dialect::Postgres).unwrap(), "CREATE TABLE IF NOT EXISTS project (
    id BIGSERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    stars INT4 NOT NULL,
    views INT8 NOT NULL,
    score FLOAT8 NOT NULL,
    done BOOL NOT NULL,
    tags TEXT[] NOT NULL,
    cover BYTEA,
    at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);
");
        let unsigned = vec![Field { name: "count".into(), ty: "u32".into() }];
        assert_eq!(
            render_migration("Project", &unsigned, false, Dialect::Postgres),
            Err("no PostgreSQL column type for count: u32".to_string())
        );
    }

    #[test]
    fn mysql_migration_uses_mysql_types() {
        let fields = vec![
            Field { name: "title".into(), ty: "String".into() },
            Field { name: "stars".into(), ty: "i32".into() },
            Field { name: "count".into(), ty: "u32".into() },
            Field { name: "score".into(), ty: "f64".into() },
            Field { name: "done".into(), ty: "bool".into() },
            Field { name: "summary".into(), ty: "Option<String>".into() },
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
        let tags = vec![Field { name: "tags".into(), ty: "Vec<String>".into() }];
        assert!(render_migration("Project", &tags, false, Dialect::Mysql).is_err());
    }

    #[test]
    fn read_fields_checks_types_against_the_database() {
        let mut output = Vec::new();
        let fields = read_fields(&mut "tags\nVec<String>\n\n".as_bytes(), &mut output, false, Some(Dialect::Postgres)).unwrap();
        assert_eq!(fields, vec![Field { name: "tags".into(), ty: "Vec<String>".into() }]);
        let mut output = Vec::new();
        let fields = read_fields(&mut "count\nu32\ni64\n\n".as_bytes(), &mut output, false, Some(Dialect::Postgres)).unwrap();
        assert_eq!(fields, vec![Field { name: "count".into(), ty: "i64".into() }]);
        assert!(String::from_utf8(output).unwrap().contains("`u32` has no PostgreSQL column type, use one of String, i16"));
    }

    #[test]
    fn postgres_queries_use_returning() {
        let fields = vec![Field { name: "title".into(), ty: "String".into() }];
        let model = render_model("Project", false, true, false, &fields, Dialect::Postgres);
        assert!(model.contains("\"INSERT INTO project (title) VALUES ($1) RETURNING *\""));
        assert!(model.contains("\"UPDATE project SET title = $1 WHERE id = $2 RETURNING id, title\""));
        assert!(model.contains("\"DELETE FROM project WHERE id = $1 RETURNING *\""));
    }

    #[test]
    fn mysql_queries_select_the_row_without_returning() {
        let fields = vec![
            Field { name: "title".into(), ty: "String".into() },
            Field { name: "stars".into(), ty: "i32".into() },
        ];
        let model = render_model("Project", false, true, true, &fields, Dialect::Mysql);
        assert!(!model.contains("RETURNING"));
        assert!(!model.contains('$'));
        assert!(model.contains("\"SELECT * FROM project WHERE id = ? AND deleted_at IS NULL\",\n            )\n            .bind(id)"));
        // save inserts then selects the inserted row
        assert!(model.contains("let now = Utc::now();\n            let result = sqlx::query(\n                \"INSERT INTO project (title, stars, created_at, updated_at) VALUES (?, ?, ?, ?)\","));
        assert!(model.contains(".execute(&state.pool)\n            .await?;\n            let model = sqlx::query_as::<_, Project>(\n                \"SELECT * FROM project WHERE id = ?\",\n            )\n            .bind(result.last_insert_id() as Id)"));
        // delete selects the row before soft deleting it
        assert!(model.contains("let now = Utc::now();\n            let mut model = sqlx::query_as::<_, Project>(\n                \"SELECT * FROM project WHERE id = ? AND deleted_at IS NULL\","));
        assert!(model.contains(".bind(self.id)\n            .fetch_one(&state.pool)\n            .await?;\n            sqlx::query(\n                \"UPDATE project SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL\",\n            )\n            .bind(now)\n            .bind(self.id)\n            .execute(&state.pool)\n            .await?;\n            model.deleted_at = Some(now);\n            Ok(model)"));
        assert!(model.contains("\"UPDATE project SET title = ?, stars = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL\""));
        assert!(model.contains("sqlx::query_as::<_, UpdatableProject>(\n                \"SELECT id, title, stars, updated_at FROM project WHERE id = ? AND deleted_at IS NULL\","));
    }
}
