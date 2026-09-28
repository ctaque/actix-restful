use sqlx::SqlitePool;

pub struct AppState {
    pub pool: SqlitePool,
}

/// Maximum number of rows returned by a list endpoint
pub const MAX_LIMIT: i64 = 100;
/// Number of rows returned by a list endpoint when no limit is given
pub const DEFAULT_LIMIT: i64 = 20;

// Clamps the pagination parameters of a list query
pub fn pagination(offset: Option<usize>, limit: Option<usize>) -> (i64, i64) {
    let offset = offset.unwrap_or(0) as i64;
    let limit = limit.map_or(DEFAULT_LIMIT, |l| (l as i64).min(MAX_LIMIT));
    (offset, limit)
}
