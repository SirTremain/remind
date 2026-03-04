//! Shared fullstack server functions.
use dioxus::prelude::*;
use eserde::{Deserialize, Serialize};

/// Echo the user input on the server.
#[post("/api/echo")]
pub async fn echo(input: String) -> Result<String, ServerFnError> {
    Ok(input)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(crate = "eserde")]
pub struct AuthUser {
    pub id: String,
    pub username: String,
    pub display_name: String,
}

/// Resolve the currently authenticated user from the session cookie.
#[post("/api/auth/session", auth: auth::Session)]
pub async fn session_user() -> Result<Option<AuthUser>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        auth::session_user(auth).await
    }
    #[cfg(not(feature = "server"))]
    {
        Err(ServerFnError::new("Auth is only available on the server"))
    }
}

/// Login with email/password.
#[post("/api/auth/login", auth: auth::Session)]
pub async fn login(email: String, password: String) -> Result<AuthUser, ServerFnError> {
    #[cfg(feature = "server")]
    {
        auth::login(auth, email, password).await
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (email, password);
        Err(ServerFnError::new("Auth is only available on the server"))
    }
}

/// Create a new account with email/password and log in immediately.
#[post("/api/auth/create-account", auth: auth::Session)]
pub async fn create_account(email: String, password: String) -> Result<AuthUser, ServerFnError> {
    #[cfg(feature = "server")]
    {
        auth::create_account(auth, email, password).await
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (email, password);
        Err(ServerFnError::new("Auth is only available on the server"))
    }
}

/// End the current login session.
#[post("/api/auth/logout", auth: auth::Session)]
pub async fn logout() -> Result<(), ServerFnError> {
    #[cfg(feature = "server")]
    {
        auth::logout(auth).await
    }
    #[cfg(not(feature = "server"))]
    {
        Err(ServerFnError::new("Auth is only available on the server"))
    }
}

/// Update the display name for the currently authenticated user.
#[post("/api/auth/display-name", auth: auth::Session)]
pub async fn update_display_name(display_name: String) -> Result<AuthUser, ServerFnError> {
    #[cfg(feature = "server")]
    {
        auth::update_display_name(auth, display_name).await
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = display_name;
        Err(ServerFnError::new("Auth is only available on the server"))
    }
}

#[cfg(not(feature = "server"))]
pub mod auth {
    #[derive(Debug, Clone, Copy)]
    pub struct Session;
}

#[cfg(feature = "server")]
pub mod auth {
    use super::AuthUser;
    use anyhow::{Context, Result};
    use argon2::password_hash::rand_core::OsRng;
    use argon2::password_hash::SaltString;
    use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
    use async_trait::async_trait;
    use axum_session::{SessionConfig, SessionLayer, SessionStore};
    use axum_session_auth::{
        AuthConfig, AuthSession, AuthSessionLayer, Authentication, HasPermission,
    };
    use axum_session_sqlx::SessionSqlitePool;
    use dioxus::prelude::ServerFnError;
    use dotenvy::dotenv;
    use eserde::{Deserialize, Serialize};
    use sqlx::SqlitePool;
    use uuid::Uuid;

    mod db;
    const COMPILETIME_DATABASE_URL: &str = dotenvy_macro::dotenv!("DATABASE_URL");

    pub type Session = AuthSession<User, String, SessionSqlitePool, SqlitePool>;
    pub type StoreLayer = SessionLayer<SessionSqlitePool>;
    pub type LoginLayer = AuthSessionLayer<User, String, SessionSqlitePool, SqlitePool>;

    #[derive(Clone)]
    pub struct AuthLayers {
        pub session_layer: StoreLayer,
        pub auth_layer: LoginLayer,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    #[serde(crate = "eserde")]
    pub struct User {
        pub id: String,
        pub username: String,
        pub display_name: String,
    }

    pub async fn build_auth_layers() -> Result<AuthLayers> {
        let _ = dotenv();
        let db_url = std::env::var("REMIND_DATABASE_URL")
            .or_else(|_| std::env::var("DATABASE_URL"))
            .unwrap_or_else(|_| COMPILETIME_DATABASE_URL.to_owned());
        let pool = db::connect_and_migrate(&db_url).await?;
        db::set_database_pool(pool.clone());

        let session_config = SessionConfig::default().with_table_name("remind_sessions");
        let session_store =
            SessionStore::<SessionSqlitePool>::new(Some(pool.clone().into()), session_config)
                .await?;

        let auth_layer =
            AuthSessionLayer::<User, String, SessionSqlitePool, SqlitePool>::new(Some(pool))
                .with_config(AuthConfig::<String>::default());
        let session_layer = SessionLayer::new(session_store);

        Ok(AuthLayers {
            session_layer,
            auth_layer,
        })
    }

    pub async fn session_user(auth: Session) -> Result<Option<AuthUser>, ServerFnError> {
        Ok(auth
            .current_user
            .as_ref()
            .filter(|user| user.is_authenticated())
            .map(AuthUser::from))
    }

    pub async fn login(
        auth: Session,
        email: String,
        password: String,
    ) -> Result<AuthUser, ServerFnError> {
        let email = email.trim().to_lowercase();
        if email.is_empty() || password.is_empty() {
            return Err(ServerFnError::new("Email and password are required"));
        }
        validate_email(&email)?;

        let pool = db::database_pool()?;
        let user = match db::find_login_user(pool, &email)
            .await
            .map_err(|err| server_error("Failed to query user account", err))?
        {
            Some(row) => {
                verify_password(&password, &row.password_hash)?;
                User {
                    id: row.id,
                    username: row.username,
                    display_name: row.display_name,
                }
            }
            None => {
                return Err(ServerFnError::new(
                    "No account found for that email. Use Create Account first.",
                ));
            }
        };

        auth.login_user(user.id.clone());
        auth.remember_user(true);

        Ok(AuthUser::from(&user))
    }

    pub async fn create_account(
        auth: Session,
        email: String,
        password: String,
    ) -> Result<AuthUser, ServerFnError> {
        let email = email.trim().to_lowercase();
        if email.is_empty() || password.is_empty() {
            return Err(ServerFnError::new("Email and password are required"));
        }
        validate_email(&email)?;

        let pool = db::database_pool()?;
        if db::find_login_user(pool, &email)
            .await
            .map_err(|err| server_error("Failed to query user account", err))?
            .is_some()
        {
            return Err(ServerFnError::new(
                "An account already exists for this email. Please log in instead.",
            ));
        }

        let user = create_user(pool, email, password).await?;
        auth.login_user(user.id.clone());
        auth.remember_user(true);
        Ok(AuthUser::from(&user))
    }

    pub async fn logout(auth: Session) -> Result<(), ServerFnError> {
        auth.logout_user();
        Ok(())
    }

    pub async fn update_display_name(
        auth: Session,
        display_name: String,
    ) -> Result<AuthUser, ServerFnError> {
        let display_name = display_name.trim().to_owned();
        if display_name.is_empty() {
            return Err(ServerFnError::new("Display name cannot be empty"));
        }

        let current_user = auth
            .current_user
            .clone()
            .filter(|user| user.is_authenticated())
            .ok_or_else(|| ServerFnError::new("Please sign in first"))?;

        let pool = db::database_pool()?;
        db::update_display_name(pool, &current_user.id, &display_name)
            .await
            .map_err(|err| server_error("Failed to update account settings", err))?;
        auth.cache_clear_user(current_user.id.clone());

        let updated_user = db::load_user_by_id(pool, &current_user.id)
            .await
            .map_err(|err| server_error("Failed to load updated account settings", err))?
            .ok_or_else(|| ServerFnError::new("Failed to load updated account settings"))?;

        Ok(AuthUser::from(&updated_user))
    }

    async fn create_user(
        pool: &SqlitePool,
        username: String,
        password: String,
    ) -> Result<User, ServerFnError> {
        if password.len() < 8 {
            return Err(ServerFnError::new(
                "Password must be at least 8 characters long",
            ));
        }

        let user_id = Uuid::new_v4().to_string();
        let password_hash = hash_password(&password)?;
        db::insert_user(pool, &user_id, &username, &username, &password_hash)
            .await
            .map_err(|err| server_error("Failed to create user account", err))?;

        db::load_user_by_id(pool, &user_id)
            .await
            .map_err(|err| server_error("Failed to load new user account", err))
            .and_then(|user| {
                user.ok_or_else(|| ServerFnError::new("Failed to load new user account"))
            })
    }

    fn hash_password(password: &str) -> Result<String, ServerFnError> {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|err| server_error("Failed to hash password", err))
    }

    fn verify_password(password: &str, hash: &str) -> Result<(), ServerFnError> {
        let hash = PasswordHash::new(hash)
            .map_err(|err| server_error("Failed to parse stored password hash", err))?;

        Argon2::default()
            .verify_password(password.as_bytes(), &hash)
            .map_err(|_| ServerFnError::new("Invalid email or password"))
    }

    fn server_error(context: &str, err: impl std::fmt::Display) -> ServerFnError {
        ServerFnError::new(format!("{context}: {err}"))
    }

    fn validate_email(email: &str) -> Result<(), ServerFnError> {
        let parts: Vec<&str> = email.split('@').collect();
        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            return Err(ServerFnError::new("Please enter a valid email address"));
        }
        if !parts[1].contains('.') {
            return Err(ServerFnError::new("Please enter a valid email address"));
        }
        Ok(())
    }

    impl From<&User> for AuthUser {
        fn from(value: &User) -> Self {
            Self {
                id: value.id.clone(),
                username: value.username.clone(),
                display_name: value.display_name.clone(),
            }
        }
    }

    #[async_trait]
    impl Authentication<User, String, SqlitePool> for User {
        async fn load_user(userid: String, pool: Option<&SqlitePool>) -> Result<User> {
            let pool = pool.context("No sqlite pool was provided to auth middleware")?;
            db::load_user_by_id(pool, &userid)
                .await?
                .ok_or_else(|| anyhow::anyhow!("User not found"))
        }

        fn is_authenticated(&self) -> bool {
            true
        }

        fn is_active(&self) -> bool {
            true
        }

        fn is_anonymous(&self) -> bool {
            false
        }
    }

    #[async_trait]
    impl HasPermission<SqlitePool> for User {
        async fn has(&self, _perm: &str, _pool: &Option<&SqlitePool>) -> bool {
            true
        }
    }
}
