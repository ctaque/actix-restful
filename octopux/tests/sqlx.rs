//! End-to-end tests of the routes of models deriving `SqlxModel`, `SqlxNewModel` and `SqlxUpdatableModel`,
//! on an in-memory SQLite database.

use octopux::{
    octopux_info, anyhow, async_trait, gen_endpoint, BeforeSave, HttpCreate, HttpFindListDelete, HttpUpdate, SqlxFilter, SqlxModel,
    SqlxNewModel, SqlxUpdatableModel,
};
use actix_web::{http::StatusCode, test, web, App};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

struct AppState {
    db: SqlitePool,
}

type Id = i64;

#[derive(Default, Deserialize)]
struct FindQuery {}
#[derive(Deserialize)]
struct ListQuery {
    offset: Option<usize>,
    limit: Option<usize>,
}
#[derive(Deserialize)]
struct DeleteQuery {}
#[derive(Deserialize)]
struct SaveQuery {}
#[derive(Deserialize)]
struct UpdateQuery {}

mod project {
    use super::*;

    #[derive(Default, Serialize, Deserialize, sqlx::FromRow, HttpFindListDelete, SqlxModel)]
    #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
    #[sqlx_model(database = "sqlite", pool = "db", timestamps, soft_delete, default_limit = 2)]
    #[octopux_info(path = "project")]
    pub struct Project {
        pub id: Id,
        pub name: String,
        pub r#type: Option<i32>,
        pub created_at: Option<DateTime<Utc>>,
        pub updated_at: Option<DateTime<Utc>>,
        pub deleted_at: Option<DateTime<Utc>>,
    }

    #[derive(Serialize, Deserialize, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "sqlite", model = "Project", pool = "db", timestamps)]
    pub struct NewProject {
        pub name: String,
        pub r#type: Option<i32>,
    }

    #[derive(Serialize, Deserialize, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Project, FindQuery, AppState)]
    #[sqlx_model(database = "sqlite", pool = "db", timestamps, soft_delete)]
    pub struct UpdatableProject {
        pub id: Id,
        pub name: String,
        pub r#type: Option<i32>,
        pub updated_at: Option<DateTime<Utc>>,
    }
}

// Without timestamps: hard deletes, and the table named by `table`
mod tag {
    use super::*;

    #[derive(Default, Serialize, Deserialize, sqlx::FromRow, HttpFindListDelete, SqlxModel)]
    #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
    #[sqlx_model(database = "sqlite", table = "tags", pool = "db")]
    #[octopux_info(path = "tag")]
    pub struct Tag {
        pub id: Id,
        pub label: String,
    }

    #[derive(Serialize, Deserialize, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "sqlite", model = "Tag", table = "tags", pool = "db")]
    pub struct NewTag {
        pub label: String,
    }

    #[derive(Serialize, Deserialize, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Tag, FindQuery, AppState)]
    #[sqlx_model(database = "sqlite", table = "tags", pool = "db")]
    pub struct UpdatableTag {
        pub id: Id,
        pub label: String,
    }
}

// `before_save`: the label is trimmed and uppercased before the insert and the update
mod label {
    use super::*;

    #[derive(Default, Serialize, Deserialize, sqlx::FromRow, HttpFindListDelete, SqlxModel)]
    #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
    #[sqlx_model(database = "sqlite", pool = "db")]
    #[octopux_info(path = "label")]
    pub struct Label {
        pub id: Id,
        pub name: String,
    }

    #[derive(Serialize, Deserialize, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "sqlite", model = "Label", pool = "db", before_save)]
    pub struct NewLabel {
        pub name: String,
    }

    #[derive(Serialize, Deserialize, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Label, FindQuery, AppState)]
    #[sqlx_model(database = "sqlite", pool = "db", before_save)]
    pub struct UpdatableLabel {
        pub id: Id,
        pub name: String,
    }

    fn normalize(name: &str) -> anyhow::Result<String> {
        let name = name.trim();
        anyhow::ensure!(!name.is_empty(), "EMPTY_NAME");
        Ok(name.to_uppercase())
    }

    #[async_trait]
    impl BeforeSave<AppState> for NewLabel {
        async fn before_save(mut self: Self, _state: &AppState) -> anyhow::Result<Self> {
            self.name = normalize(&self.name)?;
            Ok(self)
        }
    }

    #[async_trait]
    impl BeforeSave<AppState> for UpdatableLabel {
        async fn before_save(mut self: Self, _state: &AppState) -> anyhow::Result<Self> {
            self.name = normalize(&self.name)?;
            Ok(self)
        }
    }
}

use label::{Label, NewLabel, UpdatableLabel};
use project::{NewProject, Project, UpdatableProject};
use tag::{NewTag, Tag, UpdatableTag};

async fn state() -> web::Data<AppState> {
    // one connection, each connection to `sqlite::memory:` opening its own database
    let db = SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    sqlx::raw_sql(
        "CREATE TABLE project (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            type INTEGER,
            created_at DATETIME,
            updated_at DATETIME,
            deleted_at DATETIME
        );
        CREATE TABLE tags (id INTEGER PRIMARY KEY AUTOINCREMENT, label TEXT NOT NULL);
        CREATE TABLE label (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL);",
    )
    .execute(&db)
    .await
    .unwrap();
    web::Data::new(AppState { db })
}

macro_rules! app {
    ($state:expr) => {
        test::init_service(
            App::new()
                .app_data($state.clone())
                .configure(gen_endpoint!(Project, NewProject, UpdatableProject))
                .configure(gen_endpoint!(Tag, NewTag, UpdatableTag))
                .configure(gen_endpoint!(Label, NewLabel, UpdatableLabel)),
        )
        .await
    };
}

#[actix_web::test]
async fn creates_finds_updates_and_soft_deletes() {
    let state = state().await;
    let app = app!(state);

    let req = test::TestRequest::post().uri("/project").set_json(json!({ "name": "a", "type": 3 })).to_request();
    let created: Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(created["id"], 1);
    assert_eq!(created["name"], "a");
    assert_eq!(created["type"], 3);
    assert!(created["created_at"].is_string());
    assert_eq!(created["created_at"], created["updated_at"]);
    assert!(created["deleted_at"].is_null());

    let found: Value = test::call_and_read_body_json(&app, test::TestRequest::get().uri("/project/1").to_request()).await;
    assert_eq!(found, created);

    let req = test::TestRequest::put()
        .uri("/project/1")
        .set_json(json!({ "id": 1, "name": "b", "type": null, "updated_at": null }))
        .to_request();
    let updated: Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(updated["name"], "b");
    assert!(updated["type"].is_null());
    assert!(updated["updated_at"].as_str().unwrap() >= created["updated_at"].as_str().unwrap());
    assert!(updated.get("created_at").is_none());

    let deleted: Value = test::call_and_read_body_json(&app, test::TestRequest::delete().uri("/project/1").to_request()).await;
    assert_eq!(deleted["name"], "b");
    assert!(deleted["deleted_at"].is_string());

    // the row is kept, and skipped
    let resp = test::call_service(&app, test::TestRequest::get().uri("/project/1").to_request()).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let resp = test::call_service(&app, test::TestRequest::delete().uri("/project/1").to_request()).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM project").fetch_one(&state.db).await.unwrap();
    assert_eq!(rows, 1);
}

#[actix_web::test]
async fn lists_pages_of_the_live_rows() {
    let state = state().await;
    let app = app!(state);
    for name in ["a", "b", "c", "d"] {
        let req = test::TestRequest::post().uri("/project").set_json(json!({ "name": name, "type": null })).to_request();
        assert!(test::call_service(&app, req).await.status().is_success());
    }
    test::call_service(&app, test::TestRequest::delete().uri("/project/2").to_request()).await;

    let names = |page: Value| page.as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    let page: Value = test::call_and_read_body_json(&app, test::TestRequest::get().uri("/project").to_request()).await;
    assert_eq!(names(page), ["a", "c"]);
    let page: Value = test::call_and_read_body_json(&app, test::TestRequest::get().uri("/project?offset=1&limit=5").to_request()).await;
    assert_eq!(names(page), ["c", "d"]);
}

#[actix_web::test]
async fn hard_deletes_without_soft_delete() {
    let state = state().await;
    let app = app!(state);

    let req = test::TestRequest::post().uri("/tag").set_json(json!({ "label": "x" })).to_request();
    let created: Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(created, json!({ "id": 1, "label": "x" }));

    let req = test::TestRequest::put().uri("/tag/1").set_json(json!({ "id": 1, "label": "y" })).to_request();
    let updated: Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(updated, json!({ "id": 1, "label": "y" }));

    let deleted: Value = test::call_and_read_body_json(&app, test::TestRequest::delete().uri("/tag/1").to_request()).await;
    assert_eq!(deleted, json!({ "id": 1, "label": "y" }));
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tags").fetch_one(&state.db).await.unwrap();
    assert_eq!(rows, 0);
}

#[actix_web::test]
async fn before_save_transforms_the_payload() {
    let state = state().await;
    let app = app!(state);

    let req = test::TestRequest::post().uri("/label").set_json(json!({ "name": "  urgent " })).to_request();
    let created: Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(created, json!({ "id": 1, "name": "URGENT" }));

    let req = test::TestRequest::put().uri("/label/1").set_json(json!({ "id": 1, "name": "later" })).to_request();
    let updated: Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(updated, json!({ "id": 1, "name": "LATER" }));

    // an error of the hook aborts the query
    let req = test::TestRequest::post().uri("/label").set_json(json!({ "name": " " })).to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM label").fetch_one(&state.db).await.unwrap();
    assert_eq!(rows, 1);
}

// `offset`, `limit` and `expand` are not filters
#[allow(dead_code)]
#[derive(Default, Deserialize, SqlxFilter)]
#[sqlx_filter(database = "sqlite")]
struct ProjectFilter {
    offset: Option<usize>,
    limit: Option<usize>,
    name: Option<String>,
    r#type_gte: Option<i32>,
    #[sqlx_filter(column = "name", op = "like")]
    q: Option<String>,
    created_at_lt: Option<DateTime<Utc>>,
    #[sqlx_filter(skip)]
    expand: bool,
    #[sqlx_filter(sort = "name, type")]
    sort: Option<String>,
    #[sqlx_filter(sort_direction)]
    order: Option<String>,
}

// Names of the live projects matching `filter`, in its order or by `id`
async fn filtered(state: &AppState, filter: &ProjectFilter) -> anyhow::Result<Vec<String>> {
    let mut qb = sqlx::QueryBuilder::new("SELECT name FROM project WHERE deleted_at IS NULL");
    let mut has_where = true;
    filter.push_filters(&mut qb, &mut has_where);
    if !filter.push_order_by(&mut qb)? {
        qb.push(" ORDER BY id");
    }
    Ok(qb.build_query_scalar().fetch_all(&state.db).await?)
}

#[actix_web::test]
async fn filters_the_rows_on_the_set_fields() {
    let state = state().await;
    let app = app!(state);
    for (name, r#type) in [("alpha", 1), ("beta", 2), ("alphabet", 3)] {
        let req = test::TestRequest::post().uri("/project").set_json(json!({ "name": name, "type": r#type })).to_request();
        assert!(test::call_service(&app, req).await.status().is_success());
    }

    assert_eq!(filtered(&state, &ProjectFilter::default()).await.unwrap(), ["alpha", "beta", "alphabet"]);
    let by_name = ProjectFilter { name: Some("beta".into()), ..Default::default() };
    assert_eq!(filtered(&state, &by_name).await.unwrap(), ["beta"]);
    let combined = ProjectFilter { q: Some("alpha%".into()), r#type_gte: Some(2), ..Default::default() };
    assert_eq!(filtered(&state, &combined).await.unwrap(), ["alphabet"]);
    let before_now = ProjectFilter { created_at_lt: Some(Utc::now()), offset: Some(1), ..Default::default() };
    assert_eq!(filtered(&state, &before_now).await.unwrap(), ["alpha", "beta", "alphabet"]);

    // without a WHERE clause yet
    let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT COUNT(*) FROM project");
    let mut has_where = false;
    combined.push_filters(&mut qb, &mut has_where);
    assert!(has_where);
    assert_eq!(qb.sql().as_str(), "SELECT COUNT(*) FROM project WHERE type >= ? AND name LIKE ?");
    let count: i64 = qb.build_query_scalar().fetch_one(&state.db).await.unwrap();
    assert_eq!(count, 1);
}

#[actix_web::test]
async fn orders_the_rows_by_the_sort_field() {
    let state = state().await;
    let app = app!(state);
    for (name, r#type) in [("b", 1), ("a", 1), ("c", 2)] {
        let req = test::TestRequest::post().uri("/project").set_json(json!({ "name": name, "type": r#type })).to_request();
        assert!(test::call_service(&app, req).await.status().is_success());
    }
    let sorted = |sort: &str| ProjectFilter { sort: Some(sort.into()), ..Default::default() };

    assert_eq!(filtered(&state, &sorted("name")).await.unwrap(), ["a", "b", "c"]);
    assert_eq!(filtered(&state, &sorted("-type, name")).await.unwrap(), ["c", "a", "b"]);
    let combined = ProjectFilter { r#type_gte: Some(1), ..sorted("-name") };
    assert_eq!(filtered(&state, &combined).await.unwrap(), ["c", "b", "a"]);

    let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT name FROM project");
    assert!(sorted("-type,name").push_order_by(&mut qb).unwrap());
    assert_eq!(qb.sql().as_str(), "SELECT name FROM project ORDER BY type DESC, name ASC");

    // the direction orders the columns without `-`, which stay descending
    let directed = |sort: &str, order: &str| ProjectFilter { order: Some(order.into()), ..sorted(sort) };
    assert_eq!(filtered(&state, &directed("name", "desc")).await.unwrap(), ["c", "b", "a"]);
    assert_eq!(filtered(&state, &directed("name", "ASC")).await.unwrap(), ["a", "b", "c"]);
    assert_eq!(filtered(&state, &directed("-type,name", "desc")).await.unwrap(), ["c", "b", "a"]);
    let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT name FROM project");
    assert!(directed("type,-name", "desc").push_order_by(&mut qb).unwrap());
    assert_eq!(qb.sql().as_str(), "SELECT name FROM project ORDER BY type DESC, name DESC");
    // without `sort`, nothing to order
    let direction_only = ProjectFilter { order: Some("desc".into()), ..Default::default() };
    assert_eq!(filtered(&state, &direction_only).await.unwrap(), ["b", "a", "c"]);

    // only the columns of the attribute, each once
    for (sort, error) in [
        ("id", "invalid sort column `id`, use name, type"),
        ("name; DROP TABLE project", "invalid sort column `name; DROP TABLE project`, use name, type"),
        ("", "invalid sort column ``, use name, type"),
        ("name,-name", "the sort column `name` is repeated"),
    ] {
        let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT name FROM project");
        assert_eq!(sorted(sort).push_order_by(&mut qb).unwrap_err().to_string(), error);
        assert_eq!(qb.sql().as_str(), "SELECT name FROM project");
    }
    for filter in [directed("name", "down"), ProjectFilter { order: Some("".into()), ..Default::default() }] {
        let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT name FROM project");
        let error = filter.push_order_by(&mut qb).unwrap_err().to_string();
        assert!(error.starts_with("invalid sort direction"), "{}", error);
        assert_eq!(qb.sql().as_str(), "SELECT name FROM project");
    }
}

// The `project` table listed with the `filter` option, its list query being `ProjectFilter`
#[derive(sqlx::FromRow, SqlxModel)]
#[http_find_list_delete(Id, FindQuery, ProjectFilter, DeleteQuery, AppState)]
#[sqlx_model(database = "sqlite", table = "project", pool = "db", soft_delete, filter, default_limit = 2)]
struct FilteredProject {
    id: Id,
    name: String,
}

#[actix_web::test]
async fn filter_option_lists_the_filtered_and_sorted_rows() {
    use octopux::Model;
    let state = state().await;
    let app = app!(state);
    for (name, r#type) in [("b", 1), ("a", 2), ("c", 3), ("d", 4)] {
        let req = test::TestRequest::post().uri("/project").set_json(json!({ "name": name, "type": r#type })).to_request();
        assert!(test::call_service(&app, req).await.status().is_success());
    }
    test::call_service(&app, test::TestRequest::delete().uri("/project/4").to_request()).await;
    let list = |filter: ProjectFilter| {
        let state = state.clone();
        async move {
            let models = <FilteredProject as Model<Id, FindQuery, ProjectFilter, Vec<FilteredProject>, DeleteQuery, FilteredProject, AppState>>::list(&filter, &state).await?;
            anyhow::Ok(models.into_iter().map(|p| p.name).collect::<Vec<_>>())
        }
    };

    // by `id` without `sort`, paginated, skipping the deleted rows
    assert_eq!(list(ProjectFilter::default()).await.unwrap(), ["b", "a"]);
    assert_eq!(list(ProjectFilter { offset: Some(1), limit: Some(10), ..Default::default() }).await.unwrap(), ["a", "c"]);
    let sorted = ProjectFilter { sort: Some("name".into()), order: Some("desc".into()), limit: Some(10), ..Default::default() };
    assert_eq!(list(sorted).await.unwrap(), ["c", "b", "a"]);
    let filtered = ProjectFilter { r#type_gte: Some(2), sort: Some("-type".into()), ..Default::default() };
    assert_eq!(list(filtered).await.unwrap(), ["c", "a"]);
    let error = list(ProjectFilter { sort: Some("id".into()), ..Default::default() }).await.unwrap_err();
    assert_eq!(error.to_string(), "invalid sort column `id`, use name, type");
}

// The PostgreSQL and MySQL queries are only type checked: they differ from the SQLite ones
// by their placeholders, and on MySQL by the SELECT replacing RETURNING
macro_rules! typed_models {
    ($module:ident, $pool:ty, $database:literal) => {
        #[allow(dead_code)]
        mod $module {
            use super::{DeleteQuery, FindQuery, Id, ListQuery, SaveQuery, UpdateQuery};
            use octopux::{HttpCreate, HttpFindListDelete, HttpUpdate, SqlxFilter, SqlxModel, SqlxNewModel, SqlxUpdatableModel};
            use chrono::{DateTime, Utc};
            use serde::{Deserialize, Serialize};

            pub struct AppState {
                pool: $pool,
            }

            #[derive(Serialize, Deserialize, sqlx::FromRow, HttpFindListDelete, SqlxModel)]
            #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
            #[sqlx_model(database = $database, timestamps, soft_delete)]
            pub struct Item {
                id: Id,
                name: String,
                created_at: Option<DateTime<Utc>>,
                updated_at: Option<DateTime<Utc>>,
                deleted_at: Option<DateTime<Utc>>,
            }

            // SqlxModel alone, one HttpFindListDelete per module declaring its path struct
            #[derive(Serialize, Deserialize, sqlx::FromRow, SqlxModel)]
            #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
            #[sqlx_model(database = $database)]
            pub struct Hard {
                id: Id,
            }

            #[derive(Serialize, Deserialize, HttpCreate, SqlxNewModel)]
            #[http_create(SaveQuery, AppState)]
            #[sqlx_model(database = $database, model = "Item", timestamps)]
            pub struct NewItem {
                name: String,
            }

            #[derive(Serialize, Deserialize, HttpCreate, SqlxNewModel)]
            #[http_create(SaveQuery, AppState)]
            #[sqlx_model(database = $database, model = "Hard")]
            pub struct NewHard {}

            #[derive(Serialize, Deserialize, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
            #[http_update(Id, UpdateQuery, Item, FindQuery, AppState)]
            #[sqlx_model(database = $database, timestamps, soft_delete)]
            pub struct UpdatableItem {
                id: Id,
                name: String,
                updated_at: Option<DateTime<Utc>>,
            }

            #[derive(Deserialize, SqlxFilter)]
            #[sqlx_filter(database = $database)]
            pub struct ItemFilter {
                offset: Option<usize>,
                limit: Option<usize>,
                name: Option<String>,
                #[sqlx_filter(column = "name", op = "like")]
                q: Option<String>,
                id_gte: Option<Id>,
                created_at_lt: Option<DateTime<Utc>>,
                #[sqlx_filter(sort = "name, created_at")]
                sort: Option<String>,
                #[sqlx_filter(sort_direction)]
                order: Option<String>,
            }
        }
    };
}

typed_models!(postgres, sqlx::PgPool, "postgres");
typed_models!(mysql, sqlx::MySqlPool, "mysql");
