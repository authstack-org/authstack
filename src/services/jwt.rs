use anyhow::Result;
use chrono::Utc;
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ids::{AdminUserId, ApplicationId, OrganizationId, UserId};

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessTokenClaims {
    pub sub: String, // user_id
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
    pub sub: String, // user_id
    pub app_id: String,
    pub jti: String,
    pub iat: i64,
    pub exp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AdminTokenClaims {
    pub sub: String, // admin_user.id
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
    kid: String,
    jwks: serde_json::Value,
}

impl JwtService {
    pub fn new(
        private_key_pem: &str,
        public_key_pem: &str,
        access_expiry_secs: u64,
        refresh_expiry_secs: u64,
        kid: String,
    ) -> Result<Self> {
        let encoding_key = EncodingKey::from_ec_pem(private_key_pem.as_bytes())
            .map_err(|e| anyhow::anyhow!("invalid private key: {e}"))?;
        let decoding_key = DecodingKey::from_ec_pem(public_key_pem.as_bytes())
            .map_err(|e| anyhow::anyhow!("invalid public key: {e}"))?;
        let jwks = crate::jwk::jwks_from_public_pem(public_key_pem, &kid)?;
        Ok(Self {
            encoding_key,
            decoding_key,
            access_expiry_secs,
            refresh_expiry_secs,
            kid,
            jwks,
        })
    }

    pub fn jwks(&self) -> &serde_json::Value {
        &self.jwks
    }

    fn es256_header(&self) -> Header {
        let mut h = Header::new(Algorithm::ES256);
        h.kid = Some(self.kid.clone());
        h
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
        encode(&self.es256_header(), &claims, &self.encoding_key)
            .map_err(|e| anyhow::anyhow!("failed to sign access token: {e}"))
    }

    pub fn issue_refresh_token(
        &self,
        user_id: UserId,
        app_id: ApplicationId,
    ) -> Result<(String, String)> {
        let now = Utc::now().timestamp();
        let jti = Uuid::new_v4().to_string();
        let claims = RefreshTokenClaims {
            sub: user_id.to_string(),
            app_id: app_id.to_string(),
            jti: jti.clone(),
            iat: now,
            exp: now + self.refresh_expiry_secs as i64,
        };
        let token = encode(&self.es256_header(), &claims, &self.encoding_key)
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
        encode(&self.es256_header(), &claims, &self.encoding_key)
            .map_err(|e| anyhow::anyhow!("failed to sign admin token: {e}"))
    }

    pub fn verify_admin_token(&self, token: &str) -> Result<TokenData<AdminTokenClaims>> {
        let mut validation = Validation::new(Algorithm::ES256);
        validation.validate_exp = true;
        decode(token, &self.decoding_key, &validation)
            .map_err(|e| anyhow::anyhow!("invalid admin token: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{ApplicationId, OrganizationId, UserId};
    use jsonwebtoken::decode_header;

    #[test]
    fn access_token_header_has_kid_matching_jwks_and_verifies_via_ec_components() {
        use p256::ecdsa::SigningKey;
        use p256::pkcs8::EncodePrivateKey;
        use rand::rngs::OsRng;

        let signing_key = SigningKey::random(&mut OsRng);
        let priv_pem = signing_key
            .to_pkcs8_pem(p256::pkcs8::LineEnding::LF)
            .unwrap();
        use p256::pkcs8::EncodePublicKey;
        let pub_pem = signing_key
            .verifying_key()
            .to_public_key_pem(p256::pkcs8::LineEnding::LF)
            .unwrap();

        let kid = "jwt-svc-test-kid";
        let jwt = JwtService::new(
            priv_pem.as_str(),
            pub_pem.as_str(),
            3600,
            7200,
            kid.to_string(),
        )
        .unwrap();

        let token = jwt
            .issue_access_token(
                UserId::new(),
                ApplicationId::new(),
                OrganizationId::new(),
                "team",
                "member",
                "a@b.com",
            )
            .unwrap();

        let hdr = decode_header(&token).unwrap();
        assert_eq!(hdr.kid.as_deref(), Some(kid));

        let jwk = &jwt.jwks()["keys"][0];
        assert_eq!(jwk["kid"], kid);
        let x = jwk["x"].as_str().unwrap();
        let y = jwk["y"].as_str().unwrap();
        let key = DecodingKey::from_ec_components(x, y).unwrap();
        let mut validation = Validation::new(Algorithm::ES256);
        validation.validate_exp = true;
        let td = decode::<AccessTokenClaims>(&token, &key, &validation).unwrap();
        assert_eq!(td.claims.email, "a@b.com");
    }
}
