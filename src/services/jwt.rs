use anyhow::Result;
use chrono::Utc;
use jsonwebtoken::{
    decode, encode, Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ids::{AdminUserId, ApplicationId, OrganizationId, UserId};

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessTokenClaims {
    pub sub: String,       // user_id
    pub app_id: String,
    pub org_id: String,
    pub org_type: String,
    pub role: String,
    pub email: String,
    pub jti: String,
    pub iat: i64,
    pub exp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RefreshTokenClaims {
    pub sub: String,       // user_id
    pub app_id: String,
    pub jti: String,
    pub iat: i64,
    pub exp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AdminTokenClaims {
    pub sub: String,   // admin_user.id
    pub email: String,
    pub jti: String,
    pub iat: i64,
    pub exp: i64,
}

pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    access_expiry_secs: u64,
    refresh_expiry_secs: u64,
}

impl JwtService {
    pub fn new(private_key_pem: &str, public_key_pem: &str, access_expiry_secs: u64, refresh_expiry_secs: u64) -> Result<Self> {
        Ok(Self {
            encoding_key: EncodingKey::from_ec_pem(private_key_pem.as_bytes())
                .map_err(|e| anyhow::anyhow!("invalid private key: {e}"))?,
            decoding_key: DecodingKey::from_ec_pem(public_key_pem.as_bytes())
                .map_err(|e| anyhow::anyhow!("invalid public key: {e}"))?,
            access_expiry_secs,
            refresh_expiry_secs,
        })
    }

    pub fn issue_access_token(
        &self,
        user_id: UserId,
        app_id: ApplicationId,
        org_id: OrganizationId,
        org_type: &str,
        role: &str,
        email: &str,
    ) -> Result<String> {
        let now = Utc::now().timestamp();
        let claims = AccessTokenClaims {
            sub: user_id.to_string(),
            app_id: app_id.to_string(),
            org_id: org_id.to_string(),
            org_type: org_type.to_string(),
            role: role.to_string(),
            email: email.to_string(),
            jti: Uuid::new_v4().to_string(),
            iat: now,
            exp: now + self.access_expiry_secs as i64,
        };
        encode(&Header::new(Algorithm::ES256), &claims, &self.encoding_key)
            .map_err(|e| anyhow::anyhow!("failed to sign access token: {e}"))
    }

    pub fn issue_refresh_token(&self, user_id: UserId, app_id: ApplicationId) -> Result<(String, String)> {
        let now = Utc::now().timestamp();
        let jti = Uuid::new_v4().to_string();
        let claims = RefreshTokenClaims {
            sub: user_id.to_string(),
            app_id: app_id.to_string(),
            jti: jti.clone(),
            iat: now,
            exp: now + self.refresh_expiry_secs as i64,
        };
        let token = encode(&Header::new(Algorithm::ES256), &claims, &self.encoding_key)
            .map_err(|e| anyhow::anyhow!("failed to sign refresh token: {e}"))?;
        Ok((token, jti))
    }

    pub fn verify_access_token(&self, token: &str) -> Result<TokenData<AccessTokenClaims>> {
        let mut validation = Validation::new(Algorithm::ES256);
        validation.validate_exp = true;
        decode(token, &self.decoding_key, &validation)
            .map_err(|e| anyhow::anyhow!("invalid access token: {e}"))
    }

    pub fn verify_refresh_token(&self, token: &str) -> Result<TokenData<RefreshTokenClaims>> {
        let mut validation = Validation::new(Algorithm::ES256);
        validation.validate_exp = true;
        decode(token, &self.decoding_key, &validation)
            .map_err(|e| anyhow::anyhow!("invalid refresh token: {e}"))
    }

    pub fn issue_admin_token(&self, admin_id: AdminUserId, email: &str) -> Result<String> {
        let now = Utc::now().timestamp();
        let claims = AdminTokenClaims {
            sub: admin_id.to_string(),
            email: email.to_string(),
            jti: Uuid::new_v4().to_string(),
            iat: now,
            exp: now + self.access_expiry_secs as i64,
        };
        encode(&Header::new(Algorithm::ES256), &claims, &self.encoding_key)
            .map_err(|e| anyhow::anyhow!("failed to sign admin token: {e}"))
    }

    pub fn verify_admin_token(&self, token: &str) -> Result<TokenData<AdminTokenClaims>> {
        let mut validation = Validation::new(Algorithm::ES256);
        validation.validate_exp = true;
        decode(token, &self.decoding_key, &validation)
            .map_err(|e| anyhow::anyhow!("invalid admin token: {e}"))
    }
}
