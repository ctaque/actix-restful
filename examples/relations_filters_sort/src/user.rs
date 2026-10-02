
    // The application state, declared (or re-exported) at the root of the crate
    use crate::shared::AppState;
    use serde::{Serialize, Deserialize};
    use octopux::{
        HttpCreate,
        HttpFindListDelete,
        HttpUpdate,
        SqlxModel,
        SqlxNewModel,
        SqlxUpdatableModel,
        octopux_info,
        BeforeSave
    };
    use async_trait::async_trait;
    use argon2::{
        password_hash::{PasswordHasher},
        Argon2
    };
    use chrono::{DateTime, Utc};
    use apistos::ApiComponent;
    use schemars::JsonSchema;
    use octopux::gen_documented_endpoint;
    use anyhow::Result;

    #[derive(Default, Deserialize, JsonSchema, ApiComponent)]
    pub struct FindQuery {}
    #[derive(Deserialize, JsonSchema, ApiComponent)]
    pub struct ListQuery {
        /// Number of rows to skip
        pub offset: Option<usize>,
        /// Maximum number of rows to return (20 by default, 100 at most)
        pub limit: Option<usize>,
    }
    #[derive(Deserialize, JsonSchema, ApiComponent)]
    pub struct DeleteQuery {}
    #[derive(Deserialize, JsonSchema, ApiComponent)]
    pub struct SaveQuery {}
    #[derive(Deserialize, JsonSchema, ApiComponent)]
    pub struct UpdateQuery {}
    pub type Id = i64;

    #[derive(Default, Serialize, Deserialize, JsonSchema, ApiComponent, sqlx::FromRow, HttpFindListDelete, SqlxModel)]
    #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
    #[sqlx_model(database = "postgres", table = "users", timestamps, soft_delete)]
    #[octopux_info(path = "user")]
    pub struct User {
        pub id: Id,
        pub email: String,
        #[serde(skip_serializing)]
        pub password: String,
        pub created_at: Option<DateTime<Utc>>,
        pub updated_at: Option<DateTime<Utc>>,
        pub deleted_at: Option<DateTime<Utc>>,
    }
    


    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", table = "users", model = "User", timestamps, before_save)]
    pub struct NewUser {
        pub email: String,
        pub password: String,
    }

    #[async_trait]
    impl BeforeSave<AppState> for NewUser {
        async fn before_save(mut self: Self, _state: &AppState) -> Result<Self> {
            let argon2 = Argon2::default();
            self.password = argon2.hash_password(self.password.as_bytes())?.to_string();
            Ok(self)
        }
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, User, FindQuery, AppState)]
    #[sqlx_model(database = "postgres", table = "users", timestamps, soft_delete)]
    pub struct UpdatableUser {
        pub id: Id,
        pub email: String,
        pub updated_at: Option<DateTime<Utc>>,
    }

    // Registers the documented routes of the user endpoint
    // (octopux `openapi` feature), to mount with `.configure(user::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(User, NewUser, UpdatableUser)(cfg)
    }

    
