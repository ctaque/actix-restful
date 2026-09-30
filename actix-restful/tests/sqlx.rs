//! End-to-end tests of the routes of models deriving `SqlxModel`, `SqlxNewModel` and `SqlxUpdatableModel`,
//! on an in-memory SQLite database.

use actix_restful::{
    actix_restful_info, anyhow, async_trait, gen_endpoint, BeforeSave, HttpCreate, HttpFindListDelete, HttpUpdate, SqlxModel, SqlxNewModel,
    SqlxUpdatableModel,
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
    #[actix_restful_info(path = "project")]
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
    #[actix_restful_info(path = "tag")]
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
    #[actix_restful_info(path = "label")]
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

// The PostgreSQL and MySQL queries are only type checked: they differ from the SQLite ones
// by their placeholders, and on MySQL by the SELECT replacing RETURNING
macro_rules! typed_models {
    ($module:ident, $pool:ty, $database:literal) => {
        #[allow(dead_code)]
        mod $module {
            use super::{DeleteQuery, FindQuery, Id, ListQuery, SaveQuery, UpdateQuery};
            use actix_restful::{HttpCreate, HttpFindListDelete, HttpUpdate, SqlxModel, SqlxNewModel, SqlxUpdatableModel};
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
        }
    };
}

typed_models!(postgres, sqlx::PgPool, "postgres");
typed_models!(mysql, sqlx::MySqlPool, "mysql");
