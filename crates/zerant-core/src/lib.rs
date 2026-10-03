//! Shared fail-closed encoding rules for Zerant v0.1.
#![forbid(unsafe_code)]

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use url::{Host, Url};

pub type Result<T> = std::result::Result<T, Error>;
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
pub const MAX_JSON_BYTES: usize = 1_048_576;

/// Errors deliberately omit input tokens and secret material.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("invalid encoding")]
    Encoding,
    #[error("invalid or noncanonical JSON")]
    Json,
    #[error("float or unsafe integer in signed JSON")]
    UnsafeNumber,
    #[error("invalid Unix-second timestamp or validity interval")]
    Time,
    #[error("invalid canonical verifier origin")]
    Origin,
    #[error("invalid credential or claim")]
    Credential,
    #[error("invalid public Ed25519 JWK")]
    Key,
    #[error("issuer or key is not trusted for this operation")]
    Trust,
    #[error("invalid compact JWS profile or signature")]
    Signature,
    #[error("audience mismatch")]
    Audience,
    #[error("revocation snapshot is invalid, stale, or rolled back")]
    Revocation,
    #[error("credential is revoked")]
    Revoked,
    #[error("payload size limit exceeded")]
    Size,
}

pub fn encode_base64url(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn decode_base64url(input: &str) -> Result<Vec<u8>> {
    if input.is_empty()
        || !input
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(Error::Encoding);
    }
    let bytes = URL_SAFE_NO_PAD.decode(input).map_err(|_| Error::Encoding)?;
    if encode_base64url(&bytes) != input {
        return Err(Error::Encoding);
    }
    Ok(bytes)
}

pub fn decode_fixed<const N: usize>(input: &str) -> Result<[u8; N]> {
    // Bound allocation before decoding.
    if input.len() != (N * 4).div_ceil(3) {
        return Err(Error::Encoding);
    }
    decode_base64url(input)?
        .try_into()
        .map_err(|_| Error::Encoding)
}

pub fn validate_id(input: &str) -> Result<[u8; 16]> {
    decode_fixed(input)
}

pub fn validate_challenge(input: &str) -> Result<[u8; 32]> {
    decode_fixed(input)
}

pub fn validate_timestamp(time: u64) -> Result<()> {
    if time > MAX_SAFE_INTEGER {
        return Err(Error::Time);
    }
    Ok(())
}

/// No grace interval. The caller must supply a healthy clock.
pub fn validate_interval(issued_at: u64, expires_at: u64, now: u64) -> Result<()> {
    for time in [issued_at, expires_at, now] {
        validate_timestamp(time)?;
    }
    if issued_at >= expires_at || now < issued_at || now >= expires_at {
        return Err(Error::Time);
    }
    Ok(())
}

pub fn check_safe_numbers(value: &Value) -> Result<()> {
    match value {
        Value::Number(n) => {
            let safe = n.as_u64().is_some_and(|v| v <= MAX_SAFE_INTEGER)
                || n.as_i64()
                    .is_some_and(|v| v.unsigned_abs() <= MAX_SAFE_INTEGER);
            if !safe {
                return Err(Error::UnsafeNumber);
            }
        }
        Value::Array(values) => {
            for v in values {
                check_safe_numbers(v)?;
            }
        }
        Value::Object(values) => {
            for v in values.values() {
                check_safe_numbers(v)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn canonicalize<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let value = serde_json::to_value(value).map_err(|_| Error::Json)?;
    check_safe_numbers(&value)?;
    let bytes = serde_json_canonicalizer::to_vec(&value).map_err(|_| Error::Json)?;
    if bytes.len() > MAX_JSON_BYTES {
        return Err(Error::Size);
    }
    Ok(bytes)
}

/// Canonical byte equality rejects duplicate members, alternate numeric/string
/// spellings and whitespace before typed parsing. Invalid Unicode is rejected by serde.
pub fn parse_canonical<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    if bytes.len() > MAX_JSON_BYTES {
        return Err(Error::Size);
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| Error::Json)?;
    if canonicalize(&value)? != bytes {
        return Err(Error::Json);
    }
    serde_json::from_slice(bytes).map_err(|_| Error::Json)
}

/// Syntax validation only: this does not authenticate a transport origin.
/// HTTP requires both a loopback host and exact explicit configuration.
pub fn validate_origin(origin: &str, allowed_loopback: &[String]) -> Result<()> {
    if !origin.is_ascii() {
        return Err(Error::Origin);
    }
    let url = Url::parse(origin).map_err(|_| Error::Origin)?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.origin().ascii_serialization() != origin
    {
        return Err(Error::Origin);
    }
    let loopback = match url.host() {
        Some(Host::Domain("localhost")) => true,
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        _ => false,
    };
    match url.scheme() {
        "https" => Ok(()),
        "http" if loopback && allowed_loopback.iter().any(|v| v == origin) => Ok(()),
        _ => Err(Error::Origin),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn integers_are_safe_and_floats_forbidden() {
        for value in [
            json!(9_007_199_254_740_992_u64),
            json!(-9_007_199_254_740_992_i64),
            json!(1.0),
            json!({"nested": [1.5]}),
        ] {
            assert_eq!(canonicalize(&value), Err(Error::UnsafeNumber));
        }
        assert!(canonicalize(&json!([-9_007_199_254_740_991_i64, MAX_SAFE_INTEGER])).is_ok());
    }
    #[test]
    fn jcs_vector_and_duplicate_rejection() {
        assert_eq!(
            canonicalize(&json!({"z": 1, "a": "é"})).unwrap(),
            "{\"a\":\"é\",\"z\":1}".as_bytes()
        );
        for input in [
            r#"{"a":1,"a":1}"#,
            r#"{ "a":1}"#,
            r#"{"a":1e0}"#,
            r#"{"a":"\ud800"}"#,
        ] {
            assert!(parse_canonical::<Value>(input.as_bytes()).is_err());
        }
    }
    #[test]
    fn base64url_is_strict() {
        let id = encode_base64url(&[0; 16]);
        assert!(validate_id(&id).is_ok());
        for bad in [
            format!("{id}="),
            "AAAAAAAAAAAAAAAAAAAAAB".into(),
            "a".into(),
            "***".into(),
        ] {
            assert!(validate_id(&bad).is_err());
        }
        assert!(validate_challenge(&encode_base64url(&[1; 32])).is_ok());
    }
    #[test]
    fn origins_are_exact() {
        let allowed = vec!["http://localhost:3000".into(), "http://[::1]:3000".into()];
        for good in [
            "https://example.org",
            "https://example.org:444",
            "http://localhost:3000",
            "http://[::1]:3000",
        ] {
            assert!(validate_origin(good, &allowed).is_ok());
        }
        for bad in [
            "https://EXAMPLE.org",
            "https://example.org/",
            "https://example.org:443",
            "https://example.org/path",
            "https://user@example.org",
            "http://example.org",
            "http://localhost:3001",
            "https://example.org?x",
            "https://é.org",
        ] {
            assert!(validate_origin(bad, &allowed).is_err(), "{bad}");
        }
    }
    #[test]
    fn clock_boundaries_fail_closed() {
        assert!(validate_interval(10, 20, 10).is_ok());
        for times in [
            (10, 20, 20),
            (10, 20, 9),
            (10, 10, 10),
            (20, 10, 15),
            (0, MAX_SAFE_INTEGER + 1, 1),
        ] {
            assert!(validate_interval(times.0, times.1, times.2).is_err());
        }
    }
}
