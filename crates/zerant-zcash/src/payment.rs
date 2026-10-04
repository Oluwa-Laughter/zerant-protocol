//! Generic Zcash payment intents and minimal settlement receipts.
//!
//! These types intentionally avoid wallet-wide state. They can be used by any
//! Zerant application context: commerce, freelance work, grants, communities,
//! organizations, or machine-readable service payments.
#![forbid(unsafe_code)]

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Mutex;
use zerant_core::{
    Error, MAX_SAFE_INTEGER, Result, canonicalize, encode_base64url, validate_id,
    validate_interval, validate_origin, validate_timestamp,
};

pub const PAYMENT_INTENT_SCHEMA: &str = "zerant.payment.intent.v0.1";
pub const PAYMENT_RECEIPT_SCHEMA: &str = "zerant.payment.receipt.v0.1";
const MAX_RECIPIENT_LEN: usize = 1024;
const MAX_REFERENCE_LEN: usize = 256;
const MAX_CONFIRMATIONS: u64 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ZcashNetwork {
    Regtest,
    Testnet,
    Mainnet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrivacyPolicy {
    FullPrivacy,
    LegacyCompat,
    AllowRevealedAmounts,
    AllowRevealedRecipients,
    AllowRevealedSenders,
    AllowFullyTransparent,
    AllowLinkingAccountAddresses,
    NoPrivacy,
}

impl PrivacyPolicy {
    pub fn automatic_spend_allowed(self) -> bool {
        self == Self::FullPrivacy
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaymentIntent {
    pub schema: String,
    pub intent_id: String,
    pub network: ZcashNetwork,
    pub requester_origin: String,
    pub recipient: String,
    pub amount_zat: u64,
    pub min_confirmations: u64,
    pub privacy_policy: PrivacyPolicy,
    pub issued_at: u64,
    pub expires_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_commitment: Option<String>,
}

impl PaymentIntent {
    pub fn validate(
        &self,
        now: u64,
        authenticated_origin: &str,
        allowed_loopback: &[String],
    ) -> Result<()> {
        if self.schema != PAYMENT_INTENT_SCHEMA
            || self.recipient.is_empty()
            || self.recipient.len() > MAX_RECIPIENT_LEN
            || !self.recipient.is_ascii()
            || self.amount_zat == 0
            || self.amount_zat > MAX_SAFE_INTEGER
            || self.min_confirmations == 0
            || self.min_confirmations > MAX_CONFIRMATIONS
        {
            return Err(Error::Credential);
        }
        validate_id(&self.intent_id)?;
        validate_origin(&self.requester_origin, allowed_loopback)?;
        validate_origin(authenticated_origin, allowed_loopback)?;
        if self.requester_origin != authenticated_origin {
            return Err(Error::Audience);
        }
        validate_interval(self.issued_at, self.expires_at, now)?;
        if let Some(reference) = &self.reference_commitment {
            if reference.len() > MAX_REFERENCE_LEN {
                return Err(Error::Size);
            }
            zerant_core::decode_fixed::<32>(reference).map_err(|_| Error::Encoding)?;
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<String> {
        self.validate(
            self.issued_at,
            &self.requester_origin,
            std::slice::from_ref(&self.requester_origin),
        )?;
        Ok(encode_base64url(&Sha256::digest(canonicalize(self)?)))
    }

    pub fn claim_context(&self) -> Result<String> {
        Ok(format!("payment-intent:{}", self.digest()?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShieldedPool {
    Sapling,
    Orchard,
    Ironwood,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettlementStatus {
    Observed,
    Confirmed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaymentReceipt {
    pub schema: String,
    pub intent_id: String,
    pub network: ZcashNetwork,
    pub txid: String,
    pub recipient_match: bool,
    pub amount_zat: u64,
    pub confirmations: u64,
    pub observed_at: u64,
    pub pool: ShieldedPool,
    pub status: SettlementStatus,
}

impl PaymentReceipt {
    pub fn validate_against(
        &self,
        intent: &PaymentIntent,
        now: u64,
        authenticated_origin: &str,
        allowed_loopback: &[String],
    ) -> Result<()> {
        intent.validate(now, authenticated_origin, allowed_loopback)?;
        if self.schema != PAYMENT_RECEIPT_SCHEMA
            || self.intent_id != intent.intent_id
            || self.network != intent.network
            || !self.recipient_match
            || self.amount_zat != intent.amount_zat
            || self.confirmations < intent.min_confirmations
            || self.confirmations > MAX_SAFE_INTEGER
            || self.status != SettlementStatus::Confirmed
        {
            return Err(Error::Credential);
        }
        validate_txid(&self.txid)?;
        validate_timestamp(self.observed_at)?;
        if self.observed_at < intent.issued_at
            || self.observed_at >= intent.expires_at
            || self.observed_at > now
            || now - self.observed_at > 60
        {
            return Err(Error::Time);
        }
        Ok(())
    }
}

pub fn validate_txid(txid: &str) -> Result<()> {
    if txid.len() != 64 || !txid.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::Encoding);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditStatus {
    Pending,
    Credited,
    Cancelled,
}

pub trait PaymentCreditStore: Send + Sync {
    fn register(&self, intent: &PaymentIntent) -> Result<()>;
    fn status(&self, intent_id: &str) -> Result<Option<CreditStatus>>;
}

pub struct SqlitePaymentCreditStore(Mutex<Connection>);

impl SqlitePaymentCreditStore {
    fn credit_once(
        &self,
        intent: &PaymentIntent,
        receipt: &PaymentReceipt,
        now: u64,
    ) -> Result<()> {
        receipt.validate_against(
            intent,
            now,
            &intent.requester_origin,
            std::slice::from_ref(&intent.requester_origin),
        )?;
        let digest = intent.digest()?;
        let connection = self.0.lock().map_err(|_| Error::Trust)?;
        let changed = connection
            .execute(
                "UPDATE payment_credits
                 SET txid=?1,status=1
                 WHERE intent_id=?2 AND intent_digest=?3 AND status=0 AND txid IS NULL",
                params![receipt.txid, intent.intent_id, digest],
            )
            .map_err(|_| Error::Trust)?;
        if changed != 1 {
            return Err(Error::Trust);
        }
        Ok(())
    }

    pub fn cancel(&self, intent: &PaymentIntent) -> Result<()> {
        let changed = self.0.lock().map_err(|_| Error::Trust)?.execute("UPDATE payment_credits SET status=2 WHERE intent_id=?1 AND intent_digest=?2 AND status=0", params![intent.intent_id, intent.digest()?]).map_err(|_| Error::Trust)?;
        if changed != 1 {
            return Err(Error::Trust);
        }
        Ok(())
    }

    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let connection = Connection::open(path).map_err(|_| Error::Trust)?;
        Self::from_connection(connection)
    }

    pub fn in_memory() -> Result<Self> {
        let connection = Connection::open_in_memory().map_err(|_| Error::Trust)?;
        Self::from_connection(connection)
    }

    fn from_connection(connection: Connection) -> Result<Self> {
        connection
            .busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|_| Error::Trust)?;
        connection
            .execute_batch(
                "PRAGMA journal_mode=WAL;
                 CREATE TABLE IF NOT EXISTS payment_credits (
                    intent_id TEXT PRIMARY KEY NOT NULL,
                    intent_digest TEXT NOT NULL,
                    txid TEXT UNIQUE,
                    status INTEGER NOT NULL CHECK(status IN (0,1,2))
                 );",
            )
            .map_err(|_| Error::Trust)?;
        Ok(Self(Mutex::new(connection)))
    }
}

impl PaymentCreditStore for SqlitePaymentCreditStore {
    fn register(&self, intent: &PaymentIntent) -> Result<()> {
        let digest = intent.digest()?;
        let connection = self.0.lock().map_err(|_| Error::Trust)?;
        connection
            .execute(
                "INSERT INTO payment_credits(intent_id,intent_digest,txid,status)
                 VALUES (?1,?2,NULL,0)",
                params![intent.intent_id, digest],
            )
            .map_err(|_| Error::Trust)?;
        Ok(())
    }

    fn status(&self, intent_id: &str) -> Result<Option<CreditStatus>> {
        validate_id(intent_id)?;
        let connection = self.0.lock().map_err(|_| Error::Trust)?;
        let mut statement = connection
            .prepare("SELECT status FROM payment_credits WHERE intent_id=?1")
            .map_err(|_| Error::Trust)?;
        let result = statement.query_row([intent_id], |row| row.get::<_, i64>(0));
        match result {
            Ok(0) => Ok(Some(CreditStatus::Pending)),
            Ok(1) => Ok(Some(CreditStatus::Credited)),
            Ok(2) => Ok(Some(CreditStatus::Cancelled)),
            Ok(_) => Err(Error::Trust),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(_) => Err(Error::Trust),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zerant_core::encode_base64url;

    const NOW: u64 = 1_800_000_000;
    const ORIGIN: &str = "https://pay.example";

    fn intent() -> PaymentIntent {
        PaymentIntent {
            schema: PAYMENT_INTENT_SCHEMA.into(),
            intent_id: encode_base64url(&[1; 16]),
            network: ZcashNetwork::Regtest,
            requester_origin: ORIGIN.into(),
            recipient: "uregtest-recipient-placeholder".into(),
            amount_zat: 50_000,
            min_confirmations: 3,
            privacy_policy: PrivacyPolicy::FullPrivacy,
            issued_at: NOW - 10,
            expires_at: NOW + 600,
            reference_commitment: Some(encode_base64url(&[2; 32])),
        }
    }

    fn receipt() -> PaymentReceipt {
        PaymentReceipt {
            schema: PAYMENT_RECEIPT_SCHEMA.into(),
            intent_id: encode_base64url(&[1; 16]),
            network: ZcashNetwork::Regtest,
            txid: "ab".repeat(32),
            recipient_match: true,
            amount_zat: 50_000,
            confirmations: 3,
            observed_at: NOW,
            pool: ShieldedPool::Orchard,
            status: SettlementStatus::Confirmed,
        }
    }

    #[test]
    fn valid_intent_and_receipt() {
        let intent = intent();
        intent.validate(NOW, ORIGIN, &[]).unwrap();
        receipt()
            .validate_against(&intent, NOW, ORIGIN, &[])
            .unwrap();
        assert_eq!(intent.digest().unwrap().len(), 43);
    }

    #[test]
    fn intent_is_origin_amount_and_time_bound() {
        let mut value = intent();
        value.amount_zat = 0;
        assert!(value.validate(NOW, ORIGIN, &[]).is_err());

        let mut value = intent();
        value.requester_origin = "https://other.example".into();
        assert!(value.validate(NOW, ORIGIN, &[]).is_err());

        let value = intent();
        assert!(value.validate(value.expires_at, ORIGIN, &[]).is_err());
    }

    #[test]
    fn receipt_fails_mismatch_and_weak_confirmation() {
        let intent = intent();
        let mut value = receipt();
        value.amount_zat += 1;
        assert!(value.validate_against(&intent, NOW, ORIGIN, &[]).is_err());

        let mut value = receipt();
        value.recipient_match = false;
        assert!(value.validate_against(&intent, NOW, ORIGIN, &[]).is_err());

        let mut value = receipt();
        value.confirmations = 2;
        assert!(value.validate_against(&intent, NOW, ORIGIN, &[]).is_err());
    }

    #[test]
    fn full_privacy_is_only_automatic_policy() {
        assert!(PrivacyPolicy::FullPrivacy.automatic_spend_allowed());
        assert!(!PrivacyPolicy::AllowRevealedAmounts.automatic_spend_allowed());
        assert!(!PrivacyPolicy::NoPrivacy.automatic_spend_allowed());
    }

    #[test]
    fn credit_store_rejects_stale_receipt() {
        let store = SqlitePaymentCreditStore::in_memory().unwrap();
        let intent = intent();
        let receipt = receipt();
        store.register(&intent).unwrap();
        assert!(store.credit_once(&intent, &receipt, NOW + 61).is_err());
        assert_eq!(
            store.status(&intent.intent_id).unwrap(),
            Some(CreditStatus::Pending)
        );
    }

    #[test]
    fn credit_store_prevents_duplicate_intent_and_tx_credit() {
        let store = SqlitePaymentCreditStore::in_memory().unwrap();
        let intent = intent();
        let receipt = receipt();
        store.register(&intent).unwrap();
        assert!(store.register(&intent).is_err());
        store.credit_once(&intent, &receipt, NOW).unwrap();
        assert_eq!(
            store.status(&intent.intent_id).unwrap(),
            Some(CreditStatus::Credited)
        );
        assert!(store.credit_once(&intent, &receipt, NOW).is_err());

        let mut second = intent.clone();
        second.intent_id = encode_base64url(&[3; 16]);
        store.register(&second).unwrap();
        assert!(store.credit_once(&second, &receipt, NOW).is_err());
    }
}

/// Non-deserializable evidence issued only by a named-transaction observation.
/// No Debug or serialization: transaction and invoice details stay native.
pub struct VerifiedPaymentReceipt {
    receipt: PaymentReceipt,
    intent_digest: String,
    block_hash: String,
}
impl VerifiedPaymentReceipt {
    pub fn confirmations(&self) -> u64 {
        self.receipt.confirmations
    }
    pub fn observed_at(&self) -> u64 {
        self.receipt.observed_at
    }
}

impl<T: crate::RegtestTransport> crate::Adapter<T> {
    pub fn verify_intent(
        &self,
        intent: &PaymentIntent,
        txid: &str,
        now: u64,
        origin: &str,
        loopback: &[String],
    ) -> Result<VerifiedPaymentReceipt> {
        use serde_json::{Value, json};
        intent.validate(now, origin, loopback)?;
        if intent.network != ZcashNetwork::Regtest {
            return Err(Error::Trust);
        }
        validate_txid(txid)?;
        if !self.capabilities()?.receipt_verification {
            return Err(Error::Trust);
        }
        let value = self.0.call("z_viewtransaction", json!([txid]))?;
        crate::bounded(&value)?;
        let confirmations = value
            .get("confirmations")
            .and_then(Value::as_u64)
            .filter(|n| *n <= MAX_SAFE_INTEGER && *n >= intent.min_confirmations)
            .ok_or(Error::Credential)?;
        let block_hash = value
            .get("blockhash")
            .and_then(Value::as_str)
            .ok_or(Error::Credential)?;
        validate_txid(block_hash)?;
        let block_time = value
            .get("blocktime")
            .and_then(Value::as_u64)
            .ok_or(Error::Time)?;
        validate_interval(intent.issued_at, intent.expires_at, block_time)?;
        if block_time > now {
            return Err(Error::Time);
        }
        if value.get("status").and_then(Value::as_str) != Some("mined") {
            return Err(Error::Credential);
        }
        let outputs = value
            .get("outputs")
            .and_then(Value::as_array)
            .filter(|v| !v.is_empty() && v.len() <= 4096)
            .ok_or(Error::Size)?;
        let mut total = 0u64;
        let mut pool = None;
        for output in outputs {
            if output.get("address").and_then(Value::as_str) != Some(&intent.recipient) {
                continue;
            }
            if output.get("walletInternal").and_then(Value::as_bool) != Some(false) {
                return Err(Error::Credential);
            }
            pool = Some(match output.get("pool").and_then(Value::as_str) {
                Some("sapling") => ShieldedPool::Sapling,
                Some("orchard") => ShieldedPool::Orchard,
                Some("ironwood") => ShieldedPool::Ironwood,
                _ => return Err(Error::Credential),
            });
            let amount = output
                .get("valueZat")
                .and_then(Value::as_u64)
                .filter(|n| *n <= MAX_SAFE_INTEGER)
                .ok_or(Error::Credential)?;
            total = total
                .checked_add(amount)
                .filter(|n| *n <= MAX_SAFE_INTEGER)
                .ok_or(Error::UnsafeNumber)?;
            if let Some(reference) = &intent.reference_commitment
                && output.get("memoStr").and_then(Value::as_str) != Some(reference)
            {
                return Err(Error::Credential);
            }
        }
        if total != intent.amount_zat {
            return Err(Error::Credential);
        }
        let receipt = PaymentReceipt {
            schema: PAYMENT_RECEIPT_SCHEMA.into(),
            intent_id: intent.intent_id.clone(),
            network: intent.network,
            txid: txid.to_ascii_lowercase(),
            recipient_match: true,
            amount_zat: total,
            confirmations,
            observed_at: now,
            pool: pool.ok_or(Error::Credential)?,
            status: SettlementStatus::Confirmed,
        };
        Ok(VerifiedPaymentReceipt {
            receipt,
            intent_digest: intent.digest()?,
            block_hash: block_hash.to_ascii_lowercase(),
        })
    }
}
impl SqlitePaymentCreditStore {
    /// The only high-level credit path accepts non-forgeable local observation evidence.
    pub fn credit_verified(
        &self,
        intent: &PaymentIntent,
        verified: &VerifiedPaymentReceipt,
        now: u64,
        origin: &str,
        loopback: &[String],
    ) -> Result<()> {
        verified
            .receipt
            .validate_against(intent, now, origin, loopback)?;
        if verified.intent_digest != intent.digest()? {
            return Err(Error::Trust);
        }
        validate_txid(&verified.block_hash)?;
        self.credit_once(intent, &verified.receipt, now)
    }
}

/// Wallet success alone is not settlement. Missing broadcast information fails closed.
#[derive(Debug, PartialEq, Eq)]
pub enum BroadcastState {
    BuiltNotBroadcast,
    BroadcastPendingConfirmations,
}
pub fn broadcast_state(result: &serde_json::Value) -> Result<BroadcastState> {
    crate::bounded(result)?;
    let broadcast = result
        .get("broadcast")
        .and_then(serde_json::Value::as_bool)
        .ok_or(Error::Trust)?;
    let txids = result
        .get("txids")
        .and_then(serde_json::Value::as_array)
        .filter(|a| a.len() == 1)
        .ok_or(Error::Trust)?;
    let txid = txids[0].as_str().ok_or(Error::Trust)?;
    validate_txid(txid)?;
    if result
        .get("txid")
        .is_some_and(|id| id.as_str() != Some(txid))
    {
        return Err(Error::Trust);
    }
    Ok(if broadcast {
        BroadcastState::BroadcastPendingConfirmations
    } else {
        BroadcastState::BuiltNotBroadcast
    })
}

#[cfg(test)]
mod observation_tests {
    use super::*;
    use crate::{Adapter, RegtestTransport};
    use serde_json::{Value, json};
    const NOW: u64 = 1_800_000_000;
    const ORIGIN: &str = "https://merchant.example";
    struct Mock(Value, bool);
    impl RegtestTransport for Mock {
        fn call(&self, method: &str, _: Value) -> Result<Value> {
            match method {
                "rpc.discover" => Ok(
                    json!({"methods":if self.1 {vec![json!({"name":"z_viewtransaction"})]} else {vec![]}}),
                ),
                "z_viewtransaction" => Ok(self.0.clone()),
                _ => Err(Error::Trust),
            }
        }
    }
    fn intent() -> PaymentIntent {
        PaymentIntent {
            schema: PAYMENT_INTENT_SCHEMA.into(),
            intent_id: encode_base64url(&[1; 16]),
            network: ZcashNetwork::Regtest,
            requester_origin: ORIGIN.into(),
            recipient: "synthetic-recipient".into(),
            amount_zat: 10,
            min_confirmations: 3,
            privacy_policy: PrivacyPolicy::FullPrivacy,
            issued_at: NOW - 10,
            expires_at: NOW + 300,
            reference_commitment: None,
        }
    }
    fn transaction() -> Value {
        json!({"status":"mined","confirmations":3,"blockhash":"b".repeat(64),"blocktime":NOW-1,
            "outputs":[{"address":"synthetic-recipient","valueZat":10,"walletInternal":false,"pool":"orchard","account_uuid":"private","memo":"private"}]})
    }
    fn verify(value: Value) -> Result<VerifiedPaymentReceipt> {
        Adapter(Mock(value, true)).verify_intent(&intent(), &"a".repeat(64), NOW, ORIGIN, &[])
    }
    #[test]
    fn exact_verified_receipt_credit_is_single_use_and_stale_fails() {
        let i = intent();
        let verified = verify(transaction()).unwrap();
        let store = SqlitePaymentCreditStore::in_memory().unwrap();
        store.register(&i).unwrap();
        assert!(
            store
                .credit_verified(&i, &verified, NOW + 61, ORIGIN, &[])
                .is_err()
        );
        store
            .credit_verified(&i, &verified, NOW, ORIGIN, &[])
            .unwrap();
        assert!(
            store
                .credit_verified(&i, &verified, NOW, ORIGIN, &[])
                .is_err()
        );
        let mut other = i.clone();
        other.intent_id = encode_base64url(&[2; 16]);
        store.register(&other).unwrap();
        let second = Adapter(Mock(transaction(), true))
            .verify_intent(&other, &"a".repeat(64), NOW, ORIGIN, &[])
            .unwrap();
        assert!(
            store
                .credit_verified(&other, &second, NOW, ORIGIN, &[])
                .is_err()
        );
    }
    #[test]
    fn cancelled_expired_and_unsupported_fail() {
        let i = intent();
        let receipt = verify(transaction()).unwrap();
        let store = SqlitePaymentCreditStore::in_memory().unwrap();
        store.register(&i).unwrap();
        store.cancel(&i).unwrap();
        assert!(
            store
                .credit_verified(&i, &receipt, NOW, ORIGIN, &[])
                .is_err()
        );
        assert!(
            Adapter(Mock(transaction(), true))
                .verify_intent(&i, &"a".repeat(64), i.expires_at, ORIGIN, &[])
                .is_err()
        );
        assert!(
            Adapter(Mock(transaction(), false))
                .verify_intent(&i, &"a".repeat(64), NOW, ORIGIN, &[])
                .is_err()
        );
        let mut mainnet = i;
        mainnet.network = ZcashNetwork::Mainnet;
        assert!(
            Adapter(Mock(transaction(), true))
                .verify_intent(&mainnet, &"a".repeat(64), NOW, ORIGIN, &[])
                .is_err()
        );
    }
    #[test]
    fn amount_recipient_status_time_pool_and_change_mismatches_fail() {
        for amount in [0, 9, 11, MAX_SAFE_INTEGER + 1] {
            let mut v = transaction();
            v["outputs"][0]["valueZat"] = json!(amount);
            assert!(verify(v).is_err());
        }
        for (field, value) in [
            ("address", json!("wrong")),
            ("pool", json!("transparent")),
            ("pool", json!("unknown")),
            ("walletInternal", json!(true)),
            ("valueZat", json!(10.5)),
        ] {
            let mut v = transaction();
            v["outputs"][0][field] = value;
            assert!(verify(v).is_err());
        }
        for (field, value) in [
            ("status", json!("unknown")),
            ("confirmations", json!(2)),
            ("blocktime", json!(NOW - 20)),
            ("blockhash", Value::Null),
        ] {
            let mut v = transaction();
            v[field] = value;
            assert!(verify(v).is_err());
        }
        let mut v = transaction();
        let output = v["outputs"][0].clone();
        v["outputs"].as_array_mut().unwrap().push(output);
        assert!(verify(v).is_err());
    }
    #[test]
    fn operation_success_requires_explicit_broadcast_and_unambiguous_txid() {
        let mut v = json!({"txids":["a".repeat(64)],"broadcast":true});
        assert_eq!(
            broadcast_state(&v).unwrap(),
            BroadcastState::BroadcastPendingConfirmations
        );
        v["broadcast"] = json!(false);
        assert_eq!(
            broadcast_state(&v).unwrap(),
            BroadcastState::BuiltNotBroadcast
        );
        v.as_object_mut().unwrap().remove("broadcast");
        assert!(broadcast_state(&v).is_err());
        v["broadcast"] = json!(true);
        v["txids"] = json!(["a".repeat(64), "b".repeat(64)]);
        assert!(broadcast_state(&v).is_err());
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    #[test]
    fn payment_credit_is_atomic_across_connections_and_restart() {
        let path = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!(
                "payment-credit-{}-{}.sqlite",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let intent = PaymentIntent {
            schema: PAYMENT_INTENT_SCHEMA.into(),
            intent_id: encode_base64url(&[1; 16]),
            network: ZcashNetwork::Regtest,
            requester_origin: "https://merchant.example".into(),
            recipient: "synthetic".into(),
            amount_zat: 10,
            min_confirmations: 3,
            privacy_policy: PrivacyPolicy::FullPrivacy,
            issued_at: 100,
            expires_at: 200,
            reference_commitment: None,
        };
        let receipt = Arc::new(VerifiedPaymentReceipt {
            intent_digest: intent.digest().unwrap(),
            block_hash: "b".repeat(64),
            receipt: PaymentReceipt {
                schema: PAYMENT_RECEIPT_SCHEMA.into(),
                intent_id: intent.intent_id.clone(),
                network: intent.network,
                txid: "a".repeat(64),
                recipient_match: true,
                amount_zat: 10,
                confirmations: 3,
                observed_at: 101,
                pool: ShieldedPool::Orchard,
                status: SettlementStatus::Confirmed,
            },
        });
        let a = SqlitePaymentCreditStore::open(&path).unwrap();
        a.register(&intent).unwrap();
        let b = SqlitePaymentCreditStore::open(&path).unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let handles = [a, b]
            .into_iter()
            .map(|store| {
                let intent = intent.clone();
                let receipt = receipt.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    store
                        .credit_verified(&intent, &receipt, 101, &intent.requester_origin, &[])
                        .is_ok()
                })
            })
            .collect::<Vec<_>>();
        let accepted = handles
            .into_iter()
            .map(|h| usize::from(h.join().unwrap()))
            .sum::<usize>();
        assert_eq!(accepted, 1);
        let reopened = SqlitePaymentCreditStore::open(&path).unwrap();
        assert_eq!(
            reopened.status(&intent.intent_id).unwrap(),
            Some(CreditStatus::Credited)
        );
        assert!(
            reopened
                .credit_verified(&intent, &receipt, 101, &intent.requester_origin, &[])
                .is_err()
        );
        drop(reopened);
        std::fs::remove_file(path).unwrap();
    }
}
