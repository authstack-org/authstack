use anyhow::Context;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub jwt_private_key: String,
    pub jwt_public_key: String,
    pub admin_key: String,
    pub access_token_expiry_secs: u64,
    pub refresh_token_expiry_secs: u64,
    pub port: u16,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: required("DATABASE_URL")?,
            jwt_private_key: required("JWT_PRIVATE_KEY")?,
            jwt_public_key: required("JWT_PUBLIC_KEY")?,
            admin_key: required("AEGIS_ADMIN_KEY")?,
            access_token_expiry_secs: optional("ACCESS_TOKEN_EXPIRY_SECS", 900),
            refresh_token_expiry_secs: optional("REFRESH_TOKEN_EXPIRY_SECS", 2_592_000),
            port: optional("PORT", 8080),
        })
    }
}

fn required(key: &str) -> anyhow::Result<String> {
    std::env::var(key).with_context(|| format!("missing required env var: {key}"))
}

fn optional<T: std::str::FromStr + Clone>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
