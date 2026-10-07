use super::*;

const MAX_BATCH: i64 = 100;

// This inventory is intentionally explicit. Adding a VaultCipher storage family requires
// adding its account binding here before an old KEK can be retired.
struct Family {
    name: &'static str,
    table: &'static str,
    source: &'static str,
    id: &'static str,
    account: &'static str,
    // Lock joined owners too: issuer ownership transfer changes the AAD account
    // and re-encrypts child rows in one transaction.
    lock: &'static str,
    prefix: &'static str,
    scope: &'static str,
}

const FAMILIES: &[Family] = &[
    Family {
        name: "credential_envelopes",
        table: "credential_envelopes",
        source: "credential_envelopes t",
        id: "t.id",
        account: "t.account_id",
        lock: "t",
        prefix: "",
        scope: "credential",
    },
    Family {
        name: "account_credential_keys",
        table: "account_credential_keys",
        source: "account_credential_keys t",
        id: "t.account_id",
        account: "t.account_id",
        lock: "t",
        prefix: "",
        scope: "credential",
    },
    Family {
        name: "issuer_profiles",
        table: "issuer_profiles",
        source: "issuer_profiles t",
        id: "t.id",
        account: "t.account_id",
        lock: "t",
        prefix: "",
        scope: "credential",
    },
    Family {
        name: "issuer_signing_keys",
        table: "issuer_signing_keys",
        source: "issuer_signing_keys t JOIN issuer_profiles p ON p.id = t.issuer_profile_id",
        id: "t.id",
        account: "p.account_id",
        lock: "t, p",
        prefix: "",
        scope: "credential",
    },
    Family {
        name: "verifier_profiles",
        table: "verifier_profiles",
        source: "verifier_profiles t",
        id: "t.id",
        account: "t.account_id",
        lock: "t",
        prefix: "",
        scope: "credential",
    },
    Family {
        name: "verifier_signing_keys",
        table: "verifier_signing_keys",
        source: "verifier_signing_keys t JOIN verifier_profiles p ON p.id = t.verifier_profile_id",
        id: "t.id",
        account: "p.account_id",
        lock: "t, p",
        prefix: "",
        scope: "credential",
    },
    Family {
        name: "holder_pairwise_keys",
        table: "holder_pairwise_keys",
        source: "holder_pairwise_keys t",
        id: "t.id",
        account: "t.holder_account_id",
        lock: "t",
        prefix: "",
        scope: "credential",
    },
    Family {
        name: "verifier_webhooks",
        table: "verifier_webhooks",
        source: "verifier_webhooks t JOIN verifier_profiles p ON p.id = t.verifier_profile_id",
        id: "t.id",
        account: "p.account_id",
        lock: "t, p",
        prefix: "",
        scope: "credential",
    },
    Family {
        name: "verification_responses",
        table: "verification_requests",
        source: "verification_requests t JOIN verifier_profiles p ON p.id = t.verifier_profile_id",
        id: "t.id",
        account: "p.account_id",
        lock: "t, p",
        prefix: "response_",
        scope: "credential",
    },
    Family {
        name: "zcash_payout_destinations",
        table: "zcash_payout_destinations",
        source: "zcash_payout_destinations t",
        id: "t.id",
        account: "t.subject_account_id",
        lock: "t",
        prefix: "",
        scope: "payout-destination",
    },
];

fn parse_args(args: &[String]) -> Result<(&str, i32, i64), ApiError> {
    if args.len() < 3 || args.len() > 4 {
        return Err(ApiError::Invalid);
    }
    let mode = args[1].as_str();
    if !matches!(mode, "status" | "batch")
        || (mode == "status" && args.len() != 3)
        || (mode == "batch" && args.len() != 4)
    {
        return Err(ApiError::Invalid);
    }
    let old = args[2].parse::<i32>().map_err(|_| ApiError::Invalid)?;
    if old <= 0 {
        return Err(ApiError::Invalid);
    }
    let limit = if mode == "batch" {
        args[3].parse::<i64>().map_err(|_| ApiError::Invalid)?
    } else {
        0
    };
    if mode == "batch" && !(1..=MAX_BATCH).contains(&limit) {
        return Err(ApiError::Invalid);
    }
    Ok((mode, old, limit))
}

fn reencrypt(
    cipher: &VaultCipher,
    scope: &str,
    account: Uuid,
    record: &CredentialRow,
) -> Result<EncryptedCredential, ApiError> {
    if record.key_version == cipher.active_key_version() {
        return Err(ApiError::Invalid);
    }
    let mut plaintext = cipher.decrypt_scoped(scope, account, record)?;
    let encrypted = cipher.encrypt_scoped(scope, account, record.id, &plaintext);
    plaintext.fill(0);
    encrypted
}

async fn inventory(pool: &Pool, old: i32) -> Result<Vec<(&'static str, i64)>, ApiError> {
    let client = db_client(pool).await?;
    let mut counts = Vec::with_capacity(FAMILIES.len());
    for family in FAMILIES {
        let sql = format!(
            "SELECT COUNT(*) FROM {} WHERE t.{}key_version = $1",
            family.source, family.prefix
        );
        let count: i64 = client
            .query_one(&sql, &[&old])
            .await
            .map_err(|_| ApiError::Unavailable)?
            .get(0);
        counts.push((family.name, count));
    }
    Ok(counts)
}

async fn migrate_batch(
    pool: &Pool,
    cipher: &VaultCipher,
    old: i32,
    limit: i64,
) -> Result<(&'static str, usize), ApiError> {
    if old == cipher.active_key_version() || !cipher.has_key_version(old) {
        return Err(ApiError::Unavailable);
    }
    let mut client = db_client(pool).await?;
    for family in FAMILIES {
        let tx = client
            .transaction()
            .await
            .map_err(|_| ApiError::Unavailable)?;
        tx.batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '60s'")
            .await
            .map_err(|_| ApiError::Unavailable)?;
        let sql = format!(
            "SELECT {} AS record_id, {} AS aad_account, t.{}ciphertext, t.{}data_nonce, t.{}wrapped_dek, t.{}wrap_nonce, t.{}key_version \
             FROM {} WHERE t.{}key_version = $1 ORDER BY {} LIMIT $2 FOR UPDATE OF {}",
            family.id,
            family.account,
            family.prefix,
            family.prefix,
            family.prefix,
            family.prefix,
            family.prefix,
            family.source,
            family.prefix,
            family.id,
            family.lock
        );
        let rows = tx
            .query(&sql, &[&old, &limit])
            .await
            .map_err(|_| ApiError::Unavailable)?;
        if rows.is_empty() {
            tx.commit().await.map_err(|_| ApiError::Unavailable)?;
            continue;
        }
        for row in &rows {
            let id: Uuid = row.get(0);
            let account: Uuid = row.get(1);
            let record = CredentialRow {
                id,
                ciphertext: row
                    .get::<_, Option<Vec<u8>>>(2)
                    .ok_or(ApiError::Unavailable)?,
                data_nonce: row
                    .get::<_, Option<Vec<u8>>>(3)
                    .ok_or(ApiError::Unavailable)?,
                wrapped_dek: row
                    .get::<_, Option<Vec<u8>>>(4)
                    .ok_or(ApiError::Unavailable)?,
                wrap_nonce: row
                    .get::<_, Option<Vec<u8>>>(5)
                    .ok_or(ApiError::Unavailable)?,
                key_version: row.get::<_, Option<i32>>(6).ok_or(ApiError::Unavailable)?,
                created_at: OffsetDateTime::UNIX_EPOCH,
                updated_at: OffsetDateTime::UNIX_EPOCH,
            };
            let encrypted = reencrypt(cipher, family.scope, account, &record)?;
            let sql = format!(
                "UPDATE {} SET {}ciphertext = $1, {}data_nonce = $2, {}wrapped_dek = $3, {}wrap_nonce = $4, {}key_version = $5 WHERE {} = $6 AND {}key_version = $7",
                family.table,
                family.prefix,
                family.prefix,
                family.prefix,
                family.prefix,
                family.prefix,
                if family.name == "account_credential_keys" {
                    "account_id"
                } else {
                    "id"
                },
                family.prefix
            );
            let changed = tx
                .execute(
                    &sql,
                    &[
                        &encrypted.ciphertext,
                        &encrypted.data_nonce,
                        &encrypted.wrapped_dek,
                        &encrypted.wrap_nonce,
                        &encrypted.key_version,
                        &id,
                        &old,
                    ],
                )
                .await
                .map_err(|_| ApiError::Unavailable)?;
            if changed != 1 {
                return Err(ApiError::Unavailable);
            }
        }
        tx.commit().await.map_err(|_| ApiError::Unavailable)?;
        return Ok((family.name, rows.len()));
    }
    Ok(("none", 0))
}

pub(super) async fn run(pool: &Pool, args: &[String]) -> Result<(), ApiError> {
    let (mode, old, limit) = parse_args(args)?;
    if mode == "batch" {
        let cipher = VaultCipher::from_env()?;
        let (family, changed) = migrate_batch(pool, &cipher, old, limit).await?;
        println!("migrated {changed} rows in {family}");
    }
    for (family, count) in inventory(pool, old).await? {
        println!("{family}: {count}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: Uuid, encrypted: EncryptedCredential) -> CredentialRow {
        CredentialRow {
            id,
            ciphertext: encrypted.ciphertext,
            data_nonce: encrypted.data_nonce,
            wrapped_dek: encrypted.wrapped_dek,
            wrap_nonce: encrypted.wrap_nonce,
            key_version: encrypted.key_version,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    fn cipher(active: i32, with_old: bool) -> VaultCipher {
        let mut keks = BTreeMap::from([(2, Aes256Gcm::new_from_slice(&[2; 32]).unwrap())]);
        if with_old {
            keks.insert(1, Aes256Gcm::new_from_slice(&[1; 32]).unwrap());
        }
        VaultCipher::from_local_keys(keks, active).unwrap()
    }

    async fn insert_test_record(
        client: &deadpool_postgres::Client,
        old: &VaultCipher,
        family: &Family,
        id: Uuid,
        aad_account: Uuid,
        id_column: &str,
        binding: Option<(&str, Uuid)>,
    ) {
        let encrypted = old
            .encrypt_scoped(family.scope, aad_account, id, family.name.as_bytes())
            .unwrap();
        let columns = format!(
            "{}ciphertext, {}data_nonce, {}wrapped_dek, {}wrap_nonce, {}key_version",
            family.prefix, family.prefix, family.prefix, family.prefix, family.prefix
        );
        if let Some((binding_column, binding_id)) = binding {
            let sql = format!(
                "INSERT INTO {} ({id_column}, {binding_column}, {columns}) VALUES ($1,$2,$3,$4,$5,$6,$7)",
                family.table
            );
            client
                .execute(
                    &sql,
                    &[
                        &id,
                        &binding_id,
                        &encrypted.ciphertext,
                        &encrypted.data_nonce,
                        &encrypted.wrapped_dek,
                        &encrypted.wrap_nonce,
                        &encrypted.key_version,
                    ],
                )
                .await
                .unwrap();
        } else {
            let sql = format!(
                "INSERT INTO {} ({id_column}, {columns}) VALUES ($1,$2,$3,$4,$5,$6)",
                family.table
            );
            client
                .execute(
                    &sql,
                    &[
                        &id,
                        &encrypted.ciphertext,
                        &encrypted.data_nonce,
                        &encrypted.wrapped_dek,
                        &encrypted.wrap_nonce,
                        &encrypted.key_version,
                    ],
                )
                .await
                .unwrap();
        }
    }

    #[test]
    fn every_encrypted_schema_family_is_listed() {
        let migrations = [
            include_str!("../migrations/0001_server_vault.sql"),
            include_str!("../migrations/0004_trust_network.sql"),
            include_str!("../migrations/0005_verification_network.sql"),
            include_str!("../migrations/0011_pairwise_holder_keys.sql"),
            include_str!("../migrations/0012_issuer_key_lifecycle.sql"),
            include_str!("../migrations/0013_verifier_key_lifecycle.sql"),
            include_str!("../migrations/0020_verifier_webhooks.sql"),
            include_str!("../migrations/0042_private_payout_destinations.sql"),
        ];
        let schema = migrations.join("\n");
        for table in [
            "credential_envelopes",
            "account_credential_keys",
            "issuer_profiles",
            "issuer_signing_keys",
            "verifier_profiles",
            "verifier_signing_keys",
            "holder_pairwise_keys",
            "verifier_webhooks",
            "verification_requests",
            "zcash_payout_destinations",
        ] {
            assert!(FAMILIES.iter().any(|family| family.table == table));
            assert!(schema.contains(&format!("CREATE TABLE IF NOT EXISTS {table}")));
        }
        assert_eq!(FAMILIES.len(), 10);
        assert_eq!(FAMILIES[8].prefix, "response_");
        assert_eq!(FAMILIES.last().unwrap().scope, "payout-destination");
    }

    #[test]
    fn migration_reencrypts_each_family_and_fails_closed() {
        let account = Uuid::new_v4();
        let old = cipher(1, true);
        let active = cipher(2, true);
        let without_old = cipher(2, false);
        for (index, family) in FAMILIES.iter().enumerate() {
            let id = Uuid::from_u128(index as u128 + 1);
            let original = row(
                id,
                old.encrypt_scoped(family.scope, account, id, b"private-record")
                    .unwrap(),
            );
            assert_eq!(
                active
                    .decrypt_scoped(family.scope, account, &original)
                    .unwrap(),
                b"private-record"
            );
            assert!(reencrypt(&without_old, family.scope, account, &original).is_err());
            let mut changed_ciphertext = row(
                id,
                old.encrypt_scoped(family.scope, account, id, b"private-record")
                    .unwrap(),
            );
            changed_ciphertext.ciphertext[0] ^= 1;
            assert!(reencrypt(&active, family.scope, account, &changed_ciphertext).is_err());
            let mut changed_wrap = row(
                id,
                old.encrypt_scoped(family.scope, account, id, b"private-record")
                    .unwrap(),
            );
            changed_wrap.wrapped_dek[0] ^= 1;
            assert!(reencrypt(&active, family.scope, account, &changed_wrap).is_err());
            let migrated = row(
                id,
                reencrypt(&active, family.scope, account, &original).unwrap(),
            );
            assert_eq!(migrated.key_version, 2);
            assert_ne!(migrated.ciphertext, original.ciphertext);
            assert_eq!(
                without_old
                    .decrypt_scoped(family.scope, account, &migrated)
                    .unwrap(),
                b"private-record"
            );
            assert!(reencrypt(&active, family.scope, account, &migrated).is_err());
        }
    }

    #[test]
    fn batch_input_is_bounded() {
        for limit in ["0", "101", "-1", "bogus"] {
            assert!(
                parse_args(&[
                    "vault-rotation".into(),
                    "batch".into(),
                    "1".into(),
                    limit.into()
                ])
                .is_err()
            );
        }
        assert!(
            parse_args(&[
                "vault-rotation".into(),
                "batch".into(),
                "1".into(),
                "100".into()
            ])
            .is_ok()
        );
    }

    // Set ZERANT_TEST_DATABASE_URL to a disposable PostgreSQL database to exercise
    // row locks, transaction rollback, batch limits, and resume against real SQL.
    #[tokio::test]
    async fn postgres_batches_resume_and_rollback() {
        let Ok(url) = env::var("ZERANT_TEST_DATABASE_URL") else {
            eprintln!("skipping PostgreSQL vault rotation test: ZERANT_TEST_DATABASE_URL is unset");
            return;
        };
        let schema = format!("vault_rotation_{}", Uuid::new_v4().simple());
        let config = tokio_postgres::Config::from_str(&url).unwrap();
        let (setup, connection) = config.connect(NoTls).await.unwrap();
        tokio::spawn(async move {
            let _ = connection.await;
        });
        setup
            .batch_execute(&format!("CREATE SCHEMA {schema}"))
            .await
            .unwrap();
        let mut scoped_config = tokio_postgres::Config::from_str(&url).unwrap();
        scoped_config.options(format!("-c search_path={schema}"));
        let manager = Manager::from_config(
            scoped_config,
            NoTls,
            ManagerConfig {
                recycling_method: RecyclingMethod::Fast,
            },
        );
        let pool = Pool::builder(manager).max_size(2).build().unwrap();
        let client = db_client(&pool).await.unwrap();
        client.batch_execute(
            "CREATE TABLE credential_envelopes (id UUID PRIMARY KEY, account_id UUID NOT NULL, ciphertext BYTEA NOT NULL, data_nonce BYTEA NOT NULL, wrapped_dek BYTEA NOT NULL, wrap_nonce BYTEA NOT NULL, key_version INTEGER NOT NULL);
             CREATE TABLE account_credential_keys (account_id UUID PRIMARY KEY, ciphertext BYTEA, data_nonce BYTEA, wrapped_dek BYTEA, wrap_nonce BYTEA, key_version INTEGER);
             CREATE TABLE issuer_profiles (id UUID PRIMARY KEY, account_id UUID, ciphertext BYTEA, data_nonce BYTEA, wrapped_dek BYTEA, wrap_nonce BYTEA, key_version INTEGER);
             CREATE TABLE issuer_signing_keys (id UUID PRIMARY KEY, issuer_profile_id UUID, ciphertext BYTEA, data_nonce BYTEA, wrapped_dek BYTEA, wrap_nonce BYTEA, key_version INTEGER);
             CREATE TABLE verifier_profiles (id UUID PRIMARY KEY, account_id UUID, ciphertext BYTEA, data_nonce BYTEA, wrapped_dek BYTEA, wrap_nonce BYTEA, key_version INTEGER);
             CREATE TABLE verifier_signing_keys (id UUID PRIMARY KEY, verifier_profile_id UUID, ciphertext BYTEA, data_nonce BYTEA, wrapped_dek BYTEA, wrap_nonce BYTEA, key_version INTEGER);
             CREATE TABLE holder_pairwise_keys (id UUID PRIMARY KEY, holder_account_id UUID, ciphertext BYTEA, data_nonce BYTEA, wrapped_dek BYTEA, wrap_nonce BYTEA, key_version INTEGER);
             CREATE TABLE verifier_webhooks (id UUID PRIMARY KEY, verifier_profile_id UUID, ciphertext BYTEA, data_nonce BYTEA, wrapped_dek BYTEA, wrap_nonce BYTEA, key_version INTEGER);
             CREATE TABLE verification_requests (id UUID PRIMARY KEY, verifier_profile_id UUID, response_ciphertext BYTEA, response_data_nonce BYTEA, response_wrapped_dek BYTEA, response_wrap_nonce BYTEA, response_key_version INTEGER);
             CREATE TABLE zcash_payout_destinations (id UUID PRIMARY KEY, subject_account_id UUID, ciphertext BYTEA, data_nonce BYTEA, wrapped_dek BYTEA, wrap_nonce BYTEA, key_version INTEGER);"
        ).await.unwrap();
        let credential_owner = Uuid::new_v4();
        let issuer_owner = Uuid::new_v4();
        let verifier_owner = Uuid::new_v4();
        let holder_owner = Uuid::new_v4();
        let issuer_profile = Uuid::new_v4();
        let verifier_profile = Uuid::new_v4();
        let old = cipher(1, true);
        let active = cipher(2, true);
        let active_only = cipher(2, false);
        let mut records = Vec::new();

        // More than one credential row proves the limit and resume behavior.
        for _ in 0..3 {
            let id = Uuid::new_v4();
            insert_test_record(
                &client,
                &old,
                &FAMILIES[0],
                id,
                credential_owner,
                "id",
                Some(("account_id", credential_owner)),
            )
            .await;
            records.push((&FAMILIES[0], id, credential_owner, "id"));
        }
        let cases = [
            (
                &FAMILIES[1],
                credential_owner,
                credential_owner,
                "account_id",
                None,
            ),
            (
                &FAMILIES[2],
                issuer_profile,
                issuer_owner,
                "id",
                Some(("account_id", issuer_owner)),
            ),
            (
                &FAMILIES[3],
                Uuid::new_v4(),
                issuer_owner,
                "id",
                Some(("issuer_profile_id", issuer_profile)),
            ),
            (
                &FAMILIES[4],
                verifier_profile,
                verifier_owner,
                "id",
                Some(("account_id", verifier_owner)),
            ),
            (
                &FAMILIES[5],
                Uuid::new_v4(),
                verifier_owner,
                "id",
                Some(("verifier_profile_id", verifier_profile)),
            ),
            (
                &FAMILIES[6],
                Uuid::new_v4(),
                holder_owner,
                "id",
                Some(("holder_account_id", holder_owner)),
            ),
            (
                &FAMILIES[7],
                Uuid::new_v4(),
                verifier_owner,
                "id",
                Some(("verifier_profile_id", verifier_profile)),
            ),
            (
                &FAMILIES[8],
                Uuid::new_v4(),
                verifier_owner,
                "id",
                Some(("verifier_profile_id", verifier_profile)),
            ),
            (
                &FAMILIES[9],
                Uuid::new_v4(),
                holder_owner,
                "id",
                Some(("subject_account_id", holder_owner)),
            ),
        ];
        for (family, id, account, id_column, binding) in cases {
            insert_test_record(&client, &old, family, id, account, id_column, binding).await;
            records.push((family, id, account, id_column));
        }

        let initial = inventory(&pool, 1).await.unwrap();
        assert_eq!(initial.iter().map(|(_, count)| count).sum::<i64>(), 12);
        assert_eq!(initial[0].1, 3);
        assert!(initial.iter().skip(1).all(|(_, count)| *count == 1));
        let mut batches = Vec::new();
        loop {
            let (family, changed) = migrate_batch(&pool, &active, 1, 2).await.unwrap();
            if changed == 0 {
                assert_eq!(family, "none");
                break;
            }
            assert!((1..=2).contains(&changed));
            batches.push((family, changed));
        }
        assert_eq!(batches.len(), FAMILIES.len() + 1);
        assert_eq!(batches[0], ("credential_envelopes", 2));
        assert_eq!(batches[1], ("credential_envelopes", 1));
        for (batch, family) in batches.iter().skip(2).zip(FAMILIES.iter().skip(1)) {
            assert_eq!(*batch, (family.name, 1));
        }
        assert!(
            inventory(&pool, 1)
                .await
                .unwrap()
                .iter()
                .all(|(_, count)| *count == 0)
        );
        assert_eq!(
            migrate_batch(&pool, &active, 1, 2).await.unwrap(),
            ("none", 0)
        );
        for (family, id, account, id_column) in records {
            let sql = format!(
                "SELECT {}ciphertext, {}data_nonce, {}wrapped_dek, {}wrap_nonce, {}key_version FROM {} WHERE {id_column} = $1",
                family.prefix,
                family.prefix,
                family.prefix,
                family.prefix,
                family.prefix,
                family.table
            );
            let row = client.query_one(&sql, &[&id]).await.unwrap();
            let record = CredentialRow {
                id,
                ciphertext: row.get(0),
                data_nonce: row.get(1),
                wrapped_dek: row.get(2),
                wrap_nonce: row.get(3),
                key_version: row.get(4),
                created_at: OffsetDateTime::UNIX_EPOCH,
                updated_at: OffsetDateTime::UNIX_EPOCH,
            };
            assert_eq!(record.key_version, 2, "{}", family.name);
            assert_eq!(
                active_only
                    .decrypt_scoped(family.scope, account, &record)
                    .unwrap(),
                family.name.as_bytes(),
                "{}",
                family.name
            );
        }

        // A later corrupt row aborts an entire batch, including a valid earlier row.
        let valid_id = Uuid::from_u128(1);
        let tampered_id = Uuid::from_u128(2);
        insert_test_record(
            &client,
            &old,
            &FAMILIES[0],
            valid_id,
            credential_owner,
            "id",
            Some(("account_id", credential_owner)),
        )
        .await;
        insert_test_record(
            &client,
            &old,
            &FAMILIES[0],
            tampered_id,
            credential_owner,
            "id",
            Some(("account_id", credential_owner)),
        )
        .await;
        client.execute("UPDATE credential_envelopes SET ciphertext = set_byte(ciphertext, 0, get_byte(ciphertext, 0) # 1) WHERE id = $1", &[&tampered_id]).await.unwrap();
        assert!(migrate_batch(&pool, &active, 1, 2).await.is_err());
        assert_eq!(inventory(&pool, 1).await.unwrap()[0].1, 2);
        let version: i32 = client
            .query_one(
                "SELECT key_version FROM credential_envelopes WHERE id = $1",
                &[&valid_id],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(version, 1);
        drop(client);
        drop(pool);
        setup
            .batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
            .await
            .unwrap();
    }
}
