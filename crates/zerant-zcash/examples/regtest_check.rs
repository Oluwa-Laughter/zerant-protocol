//! Read-only live probe. Never prints wallet-wide response data.
use zerant_zcash::{Adapter, HttpRegtestTransport};
fn run() -> zerant_core::Result<()> {
    let transport = HttpRegtestTransport::new(
        std::env::var("Z3_REGTEST_RPC_ROUTER_USER").unwrap_or_else(|_| "zebra".into()),
        std::env::var("Z3_REGTEST_RPC_ROUTER_PASSWORD").map_err(|_| zerant_core::Error::Trust)?,
    )?;
    let adapter = Adapter(transport);
    let methods = adapter.capabilities()?.methods;
    for method in ["getblockchaininfo", "getwalletinfo"] {
        if !methods.iter().any(|m| m == method) {
            return Err(zerant_core::Error::Trust);
        }
    }
    let status = adapter.chain_status()?;
    let wallet = adapter.wallet_readiness()?;
    println!(
        "regtest blocks={} wallet={wallet:?}; no payment exercised",
        status.blocks
    );
    Ok(())
}
fn main() {
    if run().is_err() {
        eprintln!("Local Z3 regtest probe failed; no live success is claimed.");
        std::process::exit(1);
    }
}
