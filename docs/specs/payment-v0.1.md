# Payment intent and receipt v0.1

`zerant-zcash::payment` provides strict intent/receipt data and a native named-transaction
verification API. JSON protocol amounts are integer zatoshis. A payment request is
never a completed payment.

An intent has schema, random 128-bit intent_id, network, requester_origin, recipient,
amount_zat, min_confirmations, explicit privacy_policy, issued_at, expires_at and
optional reference_commitment. Unknown fields/floats/unsafe integers are rejected
by canonical parsing. The digest is SHA-256 of the validated JCS payload. Bind it
in a signed disclosure request; unsigned JSON does not authenticate the requester.
Reference commitments are opaque 32-byte random values encoded base64url, never
identity text. If supplied, the native observer requires the exact opaque memoStr.

The type can describe regtest/testnet/mainnet invoices; the implemented native observer
and all automated wallet tooling reject non-regtest intents. Only FullPrivacy may be
selected for automatic spending; other explicit policies are descriptive types and
require separate reviewed integration. No automatic spend path is currently enabled.
The wallet owns address decoding and spend readiness. Placeholder addresses in fixtures
are not valid spend destinations.

`Adapter::verify_intent` discovers receipt capability, queries only the named txid,
requires status mined, blockhash, inclusion time inside the intent validity and no
future time, adequate confirmations, then sums external Sapling/Orchard/Ironwood
outputs to the exact recipient. The sum must equal the intent amount, including all
matching outputs. Transparent/change/unknown matching outputs, malformed numbers,
missing block data and unsupported methods fail closed. Unrelated outputs/memos and
account identifiers are discarded. Confirmation policy is explicit, not finality.

The result `VerifiedPaymentReceipt` has private fields and cannot be deserialized or
fabricated through public API. `PaymentReceipt` is an untrusted wire observation shape;
shape validation does not authenticate its assertions. Only `credit_verified` is the
public credit operation. It checks intent digest, origin, expiry and a maximum 60-second
observation age. For adversarial reorg-sensitive acceptance re-observe immediately;
the clock and node are trusted local inputs. A receipt reference can correlate payments.

`SqlitePaymentCreditStore` atomically registers an immutable intent digest, transitions
pending to credited/cancelled, and rejects duplicate ID/transaction credit. Txids are
lowercase in verified receipts. Cancelled/expired intents cannot be credited. Replay
survives restart, with a five-second SQLite busy timeout. Missing ledger state fails
closed. The store retains ID/digest/txid/status only; application invoice expiry,
revocation and deletion policies remain caller responsibilities. Deleting a row must
not enable re-registration/recredit; keep payment tombstones for the business retention
period. No wallet history or credential portfolio is stored.

`broadcast_state` requires an explicit broadcast boolean, exactly one txid and a
consistent optional txid alias. Built-but-not-broadcast and broadcast-pending-confirmations
are separate states; neither establishes a settled invoice. Missing/ambiguous results
fail closed.
