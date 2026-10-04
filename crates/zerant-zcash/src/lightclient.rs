use std::time::Duration;

use serde::Serialize;
use url::Url;
use zcash_client_backend::proto::service::{
    Empty, compact_tx_streamer_client::CompactTxStreamerClient,
};
use zerant_core::{Error, MAX_SAFE_INTEGER, Result};

const MAX_STATUS_TEXT: usize = 256;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightClientNetwork {
    Mainnet,
    Testnet,
}

impl LightClientNetwork {
    fn expected_chain(self) -> &'static str {
        match self {
            Self::Mainnet => "main",
            Self::Testnet => "test",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Mainnet => "mainnet",
            Self::Testnet => "testnet",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LightClientReadiness {
    pub network: String,
    pub block_height: u64,
    pub estimated_height: u64,
    pub lag: u64,
    pub synced: bool,
    pub vendor: String,
    pub version: String,
    pub protocol_version: String,
    pub consensus_branch_id: String,
}

fn safe_text(value: String) -> Result<String> {
    if value.len() > MAX_STATUS_TEXT || value.chars().any(char::is_control) {
        return Err(Error::Size);
    }
    Ok(value)
}

pub fn validate_light_client_endpoint(raw: &str, allow_loopback_http: bool) -> Result<String> {
    if raw.is_empty() || raw.len() > 2048 {
        return Err(Error::Trust);
    }

    let url = Url::parse(raw).map_err(|_| Error::Trust)?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return Err(Error::Trust);
    }

    let host = url.host_str().ok_or(Error::Trust)?;
    let loopback = matches!(host, "localhost" | "127.0.0.1" | "::1");
    let valid_scheme =
        url.scheme() == "https" || (allow_loopback_http && loopback && url.scheme() == "http");
    if !valid_scheme {
        return Err(Error::Trust);
    }

    Ok(url.to_string())
}

pub async fn fetch_light_client_readiness(
    endpoint: &str,
    expected_network: LightClientNetwork,
    allow_loopback_http: bool,
) -> Result<LightClientReadiness> {
    let endpoint = validate_light_client_endpoint(endpoint, allow_loopback_http)?;

    let mut client =
        tokio::time::timeout(REQUEST_TIMEOUT, CompactTxStreamerClient::connect(endpoint))
            .await
            .map_err(|_| Error::Trust)?
            .map_err(|_| Error::Trust)?
            .max_decoding_message_size(64 * 1024)
            .max_encoding_message_size(16 * 1024);

    let info = tokio::time::timeout(REQUEST_TIMEOUT, client.get_lightd_info(Empty {}))
        .await
        .map_err(|_| Error::Trust)?
        .map_err(|_| Error::Trust)?
        .into_inner();

    if info.chain_name != expected_network.expected_chain()
        || info.block_height > MAX_SAFE_INTEGER
        || info.estimated_height > MAX_SAFE_INTEGER
    {
        return Err(Error::Trust);
    }

    let lag = info.estimated_height.saturating_sub(info.block_height);
    Ok(LightClientReadiness {
        network: expected_network.label().to_owned(),
        block_height: info.block_height,
        estimated_height: info.estimated_height,
        lag,
        synced: info.block_height > 0 && lag <= 2,
        vendor: safe_text(info.vendor)?,
        version: safe_text(info.version)?,
        protocol_version: safe_text(info.lightwallet_protocol_version)?,
        consensus_branch_id: safe_text(info.consensus_branch_id)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_endpoint_requires_https() {
        assert!(validate_light_client_endpoint("https://zaino.example", false).is_ok());
        assert!(validate_light_client_endpoint("http://zaino.example", false).is_err());
    }

    #[test]
    fn loopback_http_requires_explicit_opt_in() {
        assert!(validate_light_client_endpoint("http://127.0.0.1:9067", false).is_err());
        assert!(validate_light_client_endpoint("http://127.0.0.1:9067", true).is_ok());
    }

    #[test]
    fn endpoint_rejects_credentials_queries_fragments_and_paths() {
        for value in [
            "https://user:pass@zaino.example",
            "https://zaino.example/path",
            "https://zaino.example?x=1",
            "https://zaino.example/#fragment",
        ] {
            assert!(
                validate_light_client_endpoint(value, false).is_err(),
                "{value}"
            );
        }
    }
}
