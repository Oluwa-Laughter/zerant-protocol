//! Bounded stdin interface. Never prints invoice metadata or raw wallet responses.
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::Read;
use zerant_core::{Error, MAX_JSON_BYTES, Result};
use zerant_zcash::{
    Adapter, HttpRegtestTransport, RegtestTransport,
    payment::{PaymentCreditStore, PaymentIntent, SqlitePaymentCreditStore},
};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    intent: PaymentIntent,
    txid: String,
    now: u64,
    authenticated_origin: String,
}
struct Fixture;
impl RegtestTransport for Fixture {
    fn call(&self, method: &str, _: Value) -> Result<Value> {
        match method {
            "rpc.discover" => Ok(json!({"methods":[{"name":"z_viewtransaction"}]})),
            "z_viewtransaction" => Ok(
                json!({"status":"mined","confirmations":3,"blockhash":"b".repeat(64),"blocktime":1800000000,
                "outputs":[{"address":"synthetic-recipient","valueZat":100000,"walletInternal":false,"pool":"orchard"}]}),
            ),
            _ => Err(Error::Trust),
        }
    }
}
fn check(adapter: impl RegtestTransport, input: &Input) -> Result<()> {
    let verified = Adapter(adapter).verify_intent(
        &input.intent,
        &input.txid,
        input.now,
        &input.authenticated_origin,
        &[],
    )?;
    let store = SqlitePaymentCreditStore::in_memory()?;
    store.register(&input.intent)?;
    store.credit_verified(
        &input.intent,
        &verified,
        input.now,
        &input.authenticated_origin,
        &[],
    )?;
    if store
        .credit_verified(
            &input.intent,
            &verified,
            input.now,
            &input.authenticated_origin,
            &[],
        )
        .is_ok()
    {
        return Err(Error::Trust);
    }
    println!(
        "exact intent condition verified; confirmations={}; duplicate credit rejected; no portable signed receipt produced",
        verified.confirmations()
    );
    Ok(())
}
fn run() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args != ["--fixture"] && args != ["--live-regtest"] {
        return Err(Error::Encoding);
    }
    let mut bytes = Vec::new();
    std::io::stdin()
        .take((MAX_JSON_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Encoding)?;
    if bytes.len() > MAX_JSON_BYTES {
        return Err(Error::Size);
    }
    // Use exact canonical bytes to reject duplicate JSON members and unsafe numbers.
    let input: Input = zerant_core::parse_canonical(&bytes)?;
    if args == ["--fixture"] {
        println!("mode=deterministic RPC fixture; no live node or spend");
        check(Fixture, &input)
    } else {
        let transport = HttpRegtestTransport::new(
            std::env::var("Z3_REGTEST_RPC_ROUTER_USER").unwrap_or_else(|_| "zebra".into()),
            std::env::var("Z3_REGTEST_RPC_ROUTER_PASSWORD").map_err(|_| Error::Trust)?,
        )?;
        println!("mode=live isolated regtest; read-only named transaction");
        check(transport, &input)
    }
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Payment verification failed: {error}; no settlement accepted");
        std::process::exit(1);
    }
}
