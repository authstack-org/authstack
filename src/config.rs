use anyhow::Context;
use base64::{Engine, engine::general_purpose::STANDARD};

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub jwt_private_key: String,
    pub jwt_public_key: String,
    /// Stable key id for JWS `kid` and JWKS `kid` (must match).
    pub jwt_kid: String,
    pub access_token_expiry_secs: u64,
    pub refresh_token_expiry_secs: u64,
    pub port: u16,
    /// Base URL for invite links (no trailing slash), e.g. https://auth.example.com
    pub public_base_url: String,
    pub invite_expiry_secs: u64,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: required("DATABASE_URL")?,
            jwt_private_key: decode_pem_key(required("JWT_PRIVATE_KEY")?)?,
            jwt_public_key: decode_pem_key(required("JWT_PUBLIC_KEY")?)?,
            jwt_kid: optional_str("JWT_KID", "authstack-1"),
            access_token_expiry_secs: optional("ACCESS_TOKEN_EXPIRY_SECS", 900),
            refresh_token_expiry_secs: optional("REFRESH_TOKEN_EXPIRY_SECS", 2_592_000),
            port: optional("PORT", 8080),
            public_base_url: optional_str("PUBLIC_BASE_URL", "http://localhost:8080")
                .trim_end_matches('/')
                .to_string(),
            invite_expiry_secs: optional("INVITE_EXPIRY_SECS", 604_800),
        })
    }
}

/// Accept keys in two formats:
///   - Base64-encoded PEM (preferred for env vars — no spaces or newlines)
///   - Raw PEM with literal `\n` escape sequences (legacy / manual paste)
fn decode_pem_key(value: String) -> anyhow::Result<String> {
    let trimmed = value.trim();
    if let Ok(bytes) = STANDARD.decode(trimmed) {
        return String::from_utf8(bytes)
            .with_context(|| "JWT key base64 decoded but is not valid UTF-8");
    }
    // Fall back: treat as raw PEM, converting literal \n to real newlines
    Ok(value.replace("\\n", "\n"))
}

fn required(key: &str) -> anyhow::Result<String> {
    let val = std::env::var(key).with_context(|| format!("missing required env var: {key}"))?;
    anyhow::ensure!(!val.is_empty(), "required env var {key} is empty");
    Ok(val)
}

fn optional<T: std::str::FromStr + Clone>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn optional_str(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}
