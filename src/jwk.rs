//! RFC 7517 JWK construction for ES256 / P-256 public keys (PEM SPKI).

use anyhow::Context;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use p256::PublicKey;
use p256::elliptic_curve::sec1::ToEncodedPoint;
use p256::pkcs8::DecodePublicKey;
use serde_json::{Value, json};

/// Build a JWKS document (`{"keys":[...]}`) from a PEM-encoded EC P-256 public key.
pub fn jwks_from_public_pem(public_key_pem: &str, kid: &str) -> anyhow::Result<Value> {
    let key = ec_p256_jwk_from_public_pem(public_key_pem, kid)?;
    Ok(json!({ "keys": [key] }))
}

fn ec_p256_jwk_from_public_pem(public_key_pem: &str, kid: &str) -> anyhow::Result<Value> {
    let pk = PublicKey::from_public_key_pem(public_key_pem.trim())
        .with_context(|| "failed to parse JWT public key PEM for JWKS")?;
    let enc = pk.to_encoded_point(false);
    let bytes = enc.as_bytes();
    anyhow::ensure!(
        bytes.len() == 65 && bytes[0] == 0x04,
        "expected uncompressed P-256 public point"
    );
    let x = &bytes[1..33];
    let y = &bytes[33..65];
    Ok(json!({
        "kty": "EC",
        "crv": "P-256",
        "kid": kid,
        "x": URL_SAFE_NO_PAD.encode(x),
        "y": URL_SAFE_NO_PAD.encode(y),
        "use": "sig",
        "alg": "ES256",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{Algorithm, DecodingKey, Header, Validation, decode, decode_header};

    #[test]
    fn jwk_has_expected_shape_and_verifies_es256_token() {
        use p256::ecdsa::SigningKey;
        use p256::pkcs8::{EncodePrivateKey, EncodePublicKey};
        use rand::rngs::OsRng;

        let signing_key = SigningKey::random(&mut OsRng);
        let priv_pem = signing_key
            .to_pkcs8_pem(p256::pkcs8::LineEnding::LF)
            .unwrap();
        let pub_pem = signing_key
            .verifying_key()
            .to_public_key_pem(p256::pkcs8::LineEnding::LF)
            .unwrap();

        let kid = "test-kid-1";
        let jwks = jwks_from_public_pem(pub_pem.as_str(), kid).unwrap();
        let keys = jwks["keys"].as_array().unwrap();
        assert_eq!(keys.len(), 1);
        let jwk = &keys[0];
        assert_eq!(jwk["kty"], "EC");
        assert_eq!(jwk["crv"], "P-256");
        assert_eq!(jwk["kid"], kid);
        assert_eq!(jwk["use"], "sig");
        assert_eq!(jwk["alg"], "ES256");
        assert!(jwk.get("pem").is_none());

        let x = jwk["x"].as_str().unwrap();
        let y = jwk["y"].as_str().unwrap();

        let encoding_key = jsonwebtoken::EncodingKey::from_ec_pem(priv_pem.as_bytes()).unwrap();
        let claims = serde_json::json!({ "sub": "u1", "iat": 1000000000, "exp": 2000000000 });
        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some(kid.to_string());
        let token = jsonwebtoken::encode(&header, &claims, &encoding_key).unwrap();

        let hdr = decode_header(&token).unwrap();
        assert_eq!(hdr.kid.as_deref(), Some(kid));
        assert_eq!(hdr.alg, Algorithm::ES256);

        let decoding_key = DecodingKey::from_ec_components(x, y).unwrap();
        let mut validation = Validation::new(Algorithm::ES256);
        validation.validate_exp = false;
        let td = decode::<serde_json::Value>(&token, &decoding_key, &validation).unwrap();
        assert_eq!(td.claims["sub"], "u1");
    }
}
