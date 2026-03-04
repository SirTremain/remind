use super::User;
use anyhow::{Context, Result};
use dioxus::prelude::ServerFnError;
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::str::FromStr;
use std::sync::OnceLock;

static DATABASE_POOL: OnceLock<SqlitePool> = OnceLock::new();
static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, Clone)]
pub struct LoginUserRow {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub password_hash: String,
}

pub async fn connect_and_migrate(db_url: &str) -> Result<SqlitePool> {
    let connect_options = SqliteConnectOptions::from_str(db_url)
        .with_context(|| format!("Invalid sqlite database URL: {db_url}"))?
        .create_if_missing(true);

    let pool = SqlitePool::connect_with(connect_options)
        .await
        .with_context(|| format!("Failed to connect to sqlite database at {db_url}"))?;

    MIGRATOR
        .run(&pool)
        .await
        .context("Failed to run database migrations")?;

    Ok(pool)
}

pub fn set_database_pool(pool: SqlitePool) {
    let _ = DATABASE_POOL.set(pool);
}

pub fn database_pool() -> Result<&'static SqlitePool, ServerFnError> {
    DATABASE_POOL
        .get()
        .ok_or_else(|| ServerFnError::new("Auth database has not been initialized"))
}

pub async fn find_login_user(
    pool: &SqlitePool,
    username: &str,
) -> Result<Option<LoginUserRow>, sqlx::Error> {
    sqlx::query_as!(
        LoginUserRow,
        r#"
        SELECT
            id as "id!: String",
            username as "username!: String",
            display_name as "display_name!: String",
            password_hash as "password_hash!: String"
        FROM users
        WHERE username = ?
        "#,
        username
    )
    .fetch_optional(pool)
    .await
}

pub async fn insert_user(
    pool: &SqlitePool,
    user_id: &str,
    username: &str,
    display_name: &str,
    password_hash: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO users (id, username, display_name, password_hash)
        VALUES (?, ?, ?, ?)
        "#,
        user_id,
        username,
        display_name,
        password_hash
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn load_user_by_id(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as!(
        User,
        r#"
        SELECT
            id as "id!: String",
            username as "username!: String",
            display_name as "display_name!: String"
        FROM users
        WHERE id = ?
        "#,
        user_id
    )
    .fetch_optional(pool)
    .await
}

pub async fn update_display_name(
    pool: &SqlitePool,
    user_id: &str,
    display_name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE users
        SET display_name = ?
        WHERE id = ?
        "#,
        display_name,
        user_id
    )
    .execute(pool)
    .await?;

    Ok(())
}
