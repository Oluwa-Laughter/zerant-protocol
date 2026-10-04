//! Zcash/Z3 integration boundary for Zerant.
//!
//! The current implementation is deliberately read-only and regtest-oriented.
//! It discovers current Z3/Zallet capabilities without accepting wallet secrets or
//! exporting wallet-wide balances/history. Payment sending remains disabled until a
//! live supported regtest contract is exercised.
#![forbid(unsafe_code)]

use serde_json::{Value, json};
use zerant_core::{Error, MAX_JSON_BYTES, MAX_SAFE_INTEGER, Result};
use zerant_credential::{ClaimValue, OrdinaryClaim};

pub const RPC_DISCOVER: &str = "rpc.discover";
pub const RPC_BLOCKCHAIN_INFO: &str = "getblockchaininfo";
pub const RPC_WALLET_INFO: &str = "getwalletinfo";
pub const RPC_SEND_MANY: &str = "z_sendmany";

/// Transport owns authentication, deadlines, endpoint selection, TLS/loopback policy,
/// and response-size enforcement before returning JSON.
pub trait RegtestTransport {
    fn call(&self, method: &str, params: Value) -> Result<Value>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    pub methods: Vec<String>,
    pub sendmany_advertised: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainStatus {
    pub blocks: u64,
    pub initial_block_download: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WalletReadiness {
    RpcReachable,
    KeyStorePresent,
}

pub struct Adapter<T>(pub T);

fn bounded(value: &Value) -> Result<()> {
    if serde_json::to_vec(value).map_err(|_| Error::Json)?.len() > MAX_JSON_BYTES {
        return Err(Error::Size);
    }
    Ok(())
}

impl<T: RegtestTransport> Adapter<T> {
    pub fn capabilities(&self) -> Result<Capabilities> {
        let response = self.0.call(RPC_DISCOVER, json!([]))?;
        bounded(&response)?;
        let methods = response
            .get("methods")
            .and_then(Value::as_array)
            .ok_or(Error::Json)?;
        if methods.len() > 1024 {
            return Err(Error::Size);
        }

        let mut names = Vec::with_capacity(methods.len());
        for method in methods {
            let name = method
                .get("name")
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty() && name.len() <= 256)
                .ok_or(Error::Json)?;
            names.push(name.to_owned());
        }
        names.sort();
        names.dedup();

        Ok(Capabilities {
            sendmany_advertised: names.iter().any(|name| name == RPC_SEND_MANY),
            methods: names,
        })
    }

    pub fn chain_status(&self) -> Result<ChainStatus> {
        let response = self.0.call(RPC_BLOCKCHAIN_INFO, json!([]))?;
        bounded(&response)?;

        // Z3 regtest documentation notes Zebra may serialize chain as "test".
        // Network authority comes from the explicitly configured local regtest
        // deployment, never this label alone.
        if !matches!(
            response.get("chain").and_then(Value::as_str),
            Some("regtest" | "test")
        ) {
            return Err(Error::Trust);
        }

        let blocks = response
            .get("blocks")
            .and_then(Value::as_u64)
            .filter(|height| *height <= MAX_SAFE_INTEGER)
            .ok_or(Error::Json)?;
        let initial_block_download = match response.get("initialblockdownload") {
            None => None,
            Some(Value::Bool(value)) => Some(*value),
            _ => return Err(Error::Json),
        };

        Ok(ChainStatus {
            blocks,
            initial_block_download,
        })
    }

    pub fn wallet_readiness(&self) -> Result<WalletReadiness> {
        let response = self.0.call(RPC_WALLET_INFO, json!([]))?;
        bounded(&response)?;
        if !response.is_object()
            || response
                .get("walletversion")
                .and_then(Value::as_u64)
                .is_none()
        {
            return Err(Error::Json);
        }

        // Do not expose balances, tx counts, addresses, or seed fingerprints.
        // Presence of wallet metadata is not proof of spend readiness.
        match response.get("mnemonic_seedfps") {
            Some(Value::Array(values)) if values.iter().all(Value::is_string) => {
                Ok(WalletReadiness::KeyStorePresent)
            }
            None => Ok(WalletReadiness::RpcReachable),
            _ => Err(Error::Json),
        }
    }
}

/// Construct the *minimal claim shape* an authorized invoice/payment issuer may sign
/// after independently validating its own expected recipient, amount, network,
/// transaction binding, confirmation/reorg policy, and business context.
///
/// No transaction ID, address, amount, balance, memo, or wallet history is embedded.
pub fn paid_invoice_claim(context: &str) -> Result<OrdinaryClaim> {
    if context.is_empty() || context.len() > 256 {
        return Err(Error::Credential);
    }
    Ok(OrdinaryClaim {
        claim_type: "payment.invoice_paid".into(),
        value: ClaimValue::Boolean(true),
        context: Some(context.into()),
    })
}

/// Optional organizational threshold-signing boundary. A future implementation may
/// delegate to reviewed Zcash Foundation FROST tooling. Zerant implements no FROST
/// cryptography here.
pub trait ThresholdSigner {
    fn sign_authorized_payload(&self, canonical_payload: &[u8], key_id: &str) -> Result<Vec<u8>>;
}

/// Concrete HTTP client restricted to the official loopback regtest router.
/// No redirects, remote endpoints, key export or spending RPCs are accepted.
pub struct HttpRegtestTransport {
    client: reqwest::blocking::Client,
    username: String,
    password: String,
}
impl HttpRegtestTransport {
    pub fn new(username: String, password: String) -> Result<Self> {
        if username.is_empty() || password.is_empty() {
            return Err(Error::Trust);
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .build()
            .map_err(|_| Error::Trust)?;
        Ok(Self {
            client,
            username,
            password,
        })
    }
}
impl RegtestTransport for HttpRegtestTransport {
    fn call(&self, method: &str, params: Value) -> Result<Value> {
        use std::io::Read;
        if !matches!(
            method,
            "rpc.discover" | "getblockchaininfo" | "getwalletinfo"
        ) {
            return Err(Error::Trust);
        }
        if params != json!([]) {
            return Err(Error::Encoding);
        }
        let response = self
            .client
            .post("http://127.0.0.1:8181")
            .basic_auth(&self.username, Some(&self.password))
            .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
            .send()
            .map_err(|_| Error::Trust)?;
        if !response.status().is_success() {
            return Err(Error::Trust);
        }
        let mut bytes = Vec::new();
        response
            .take((MAX_JSON_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Trust)?;
        if bytes.len() > MAX_JSON_BYTES {
            return Err(Error::Size);
        }
        let envelope: Value = serde_json::from_slice(&bytes).map_err(|_| Error::Json)?;
        if envelope.get("id") != Some(&json!(1))
            || envelope.get("error").is_some_and(|e| !e.is_null())
        {
            return Err(Error::Trust);
        }
        envelope.get("result").cloned().ok_or(Error::Json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Mock;

    impl RegtestTransport for Mock {
        fn call(&self, method: &str, _params: Value) -> Result<Value> {
            match method {
                RPC_DISCOVER => Ok(json!({
                    "methods":[
                        {"name":"getblockchaininfo"},
                        {"name":"getwalletinfo"},
                        {"name":"z_sendmany"}
                    ]
                })),
                RPC_BLOCKCHAIN_INFO => {
                    Ok(json!({"chain":"test","blocks":2,"initialblockdownload":false}))
                }
                RPC_WALLET_INFO => Ok(json!({
                    "walletversion":0,
                    "mnemonic_seedfps":["redacted-by-adapter"],
                    "balance":123,
                    "txcount":99
                })),
                _ => Err(Error::Credential),
            }
        }
    }

    #[test]
    fn documented_rpc_projection_is_minimal() {
        let adapter = Adapter(Mock);
        let capabilities = adapter.capabilities().unwrap();
        assert!(capabilities.sendmany_advertised);
        assert!(
            capabilities
                .methods
                .contains(&RPC_BLOCKCHAIN_INFO.to_string())
        );

        let chain = adapter.chain_status().unwrap();
        assert_eq!(chain.blocks, 2);
        assert_eq!(chain.initial_block_download, Some(false));

        assert_eq!(
            adapter.wallet_readiness().unwrap(),
            WalletReadiness::KeyStorePresent
        );
    }

    #[test]
    fn minimal_payment_claim_has_no_wallet_or_transaction_data() {
        let serialized =
            serde_json::to_string(&paid_invoice_claim("invoice:single-audience").unwrap()).unwrap();
        for forbidden in [
            "txid", "address", "amount", "balance", "memo", "seed", "wallet", "history",
        ] {
            assert!(!serialized.contains(forbidden));
        }
    }

    #[test]
    fn untrusted_network_label_fails_closed() {
        struct WrongNetwork;
        impl RegtestTransport for WrongNetwork {
            fn call(&self, method: &str, _params: Value) -> Result<Value> {
                if method == RPC_BLOCKCHAIN_INFO {
                    Ok(json!({"chain":"main","blocks":1,"initialblockdownload":false}))
                } else {
                    Err(Error::Credential)
                }
            }
        }
        assert!(Adapter(WrongNetwork).chain_status().is_err());
    }
}
