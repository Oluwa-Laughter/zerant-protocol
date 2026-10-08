//! Discoverable passkey sign-in still requires a server-stored, single-use ceremony.
//! The authenticator-supplied account handle is only a lookup hint until its
//! registered credential verifies the exact WebAuthn challenge and origin.

use super::*;

pub(super) async fn start(State(state): State<AppState>) -> Result<Response, ApiError> {
    let (mut public_key, authentication) = state
        .webauthn
        .start_discoverable_authentication()
        .map_err(|_| ApiError::Unavailable)?;
    // This endpoint is launched from an explicit button, not conditional
    // autofill on an input. Do not force conditional mediation here.
    public_key.mediation = None;
    let authentication_state =
        serde_json::to_value(authentication).map_err(|_| ApiError::Unavailable)?;
    let (attempt, _) =
        persist_passkey_challenge(&state.db, Uuid::nil(), "discoverable", authentication_state)
            .await?;
    let mut response = Json(PasskeyAuthenticationStartResponse { public_key }).into_response();
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&passkey_attempt_cookie(&attempt))
            .map_err(|_| ApiError::Unavailable)?,
    );
    Ok(response)
}

pub(super) async fn finish(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<PublicKeyCredential>,
) -> Result<Response, ApiError> {
    let attempt = cookie_value(&headers, PASSKEY_ATTEMPT_COOKIE)?;
    let attempt_hash = Sha256::digest(attempt.as_bytes()).to_vec();
    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let row = tx
        .query_opt(
            "SELECT state FROM passkey_challenges
         WHERE attempt_hash = $1 AND account_id = $2
           AND kind = 'discoverable' AND consumed_at IS NULL
           AND expires_at > NOW() FOR UPDATE",
            &[&attempt_hash, &Uuid::nil()],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Unauthorized)?;
    let state_value: Value = row.get(0);
    let authentication: DiscoverableAuthentication =
        serde_json::from_value(state_value).map_err(|_| ApiError::Unavailable)?;

    let (account, credential_bytes) = state
        .webauthn
        .identify_discoverable_authentication(&input)
        .map_err(|_| ApiError::Unauthorized)?;
    if account.is_nil() {
        return Err(ApiError::Unauthorized);
    }
    let credential_id = URL_SAFE_NO_PAD.encode(credential_bytes);
    let credential_row = tx
        .query_opt(
            "SELECT id, passkey FROM passkey_credentials
         WHERE account_id = $1 AND credential_id = $2 FOR UPDATE",
            &[&account, &credential_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Unauthorized)?;
    let passkey_id: Uuid = credential_row.get(0);
    let passkey_value: Value = credential_row.get(1);
    let mut passkey: Passkey =
        serde_json::from_value(passkey_value).map_err(|_| ApiError::Unavailable)?;
    let discoverable = DiscoverableKey::from(passkey.clone());
    let result = state
        .webauthn
        .finish_discoverable_authentication(&input, authentication, &[discoverable])
        .map_err(|_| ApiError::Unauthorized)?;
    if URL_SAFE_NO_PAD.encode(result.cred_id().as_ref()) != credential_id {
        return Err(ApiError::Unauthorized);
    }

    if passkey.update_credential(&result) == Some(true) {
        let updated = serde_json::to_value(&passkey).map_err(|_| ApiError::Unavailable)?;
        tx.execute(
            "UPDATE passkey_credentials
             SET passkey = $2, last_used_at = NOW(), updated_at = NOW()
             WHERE id = $1",
            &[&passkey_id, &updated],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    } else {
        tx.execute(
            "UPDATE passkey_credentials SET last_used_at = NOW() WHERE id = $1",
            &[&passkey_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    }
    tx.execute(
        "UPDATE passkey_challenges SET consumed_at = NOW() WHERE attempt_hash = $1",
        &[&attempt_hash],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;
    let scopes = vec!["auth".to_owned()];
    let token = create_account_session(&tx, account, &scopes, "passkey").await?;
    let row = tx
        .query_one(
            "SELECT public_handle FROM accounts WHERE id = $1",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let zerant_id: Option<String> = row.get(0);
    let zerant_id = zerant_id.ok_or(ApiError::Unavailable)?;
    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

    let mut response = Json(Authenticated {
        authenticated: true,
        identity: zerant_id.clone(),
        zerant_id,
        scopes,
    })
    .into_response();
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&session_cookie_header(&token)).map_err(|_| ApiError::Unavailable)?,
    );
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&clear_cookie_header(PASSKEY_ATTEMPT_COOKIE))
            .map_err(|_| ApiError::Unavailable)?,
    );
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Run with ZERANT_TEST_DATABASE_URL pointing at a disposable PostgreSQL database.
    #[tokio::test]
    async fn discoverable_start_stores_anonymous_one_time_ceremony() {
        let Ok(database_url) = env::var("ZERANT_TEST_DATABASE_URL") else {
            return;
        };
        let config = tokio_postgres::Config::from_str(&database_url).unwrap();
        let manager = Manager::from_config(
            config,
            NoTls,
            ManagerConfig {
                recycling_method: RecyclingMethod::Fast,
            },
        );
        let db = Pool::builder(manager).max_size(8).build().unwrap();
        run_migrations(&db).await.unwrap();
        let origin = url::Url::parse("https://zerant.example").unwrap();
        let webauthn = WebauthnBuilder::new("zerant.example", &origin)
            .unwrap()
            .rp_name("Zerant")
            .build()
            .unwrap();
        let state = AppState {
            db: db.clone(),
            cipher: Arc::new(
                VaultCipher::from_local_keys(
                    BTreeMap::from([(1, Aes256Gcm::new_from_slice(&[7; 32]).unwrap())]),
                    1,
                )
                .unwrap(),
            ),
            webauthn: Arc::new(webauthn),
            public_origin: "https://zerant.example".into(),
            zcash_chain: "zcash:testnet".into(),
            light_client_endpoints: Vec::new(),
            light_client_allow_loopback: false,
            light_client_cache: Arc::new(tokio::sync::Mutex::new(None)),
            allowed_scopes: BTreeSet::from(["auth".into()]),
        };
        let response = start(State(state)).await.unwrap();
        let cookie = response
            .headers()
            .get(SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));
        let attempt = cookie
            .split(';')
            .next()
            .unwrap()
            .strip_prefix("zerant_passkey_attempt=")
            .unwrap();
        let hash = Sha256::digest(attempt.as_bytes()).to_vec();
        let client = db_client(&db).await.unwrap();
        let row = client
            .query_one(
                "SELECT account_id, kind, state, expires_at > NOW(), consumed_at IS NULL
             FROM passkey_challenges WHERE attempt_hash = $1",
                &[&hash],
            )
            .await
            .unwrap();
        assert_eq!(row.get::<_, Uuid>(0), Uuid::nil());
        assert_eq!(row.get::<_, String>(1), "discoverable");
        assert!(row.get::<_, Value>(2).is_object());
        assert!(row.get::<_, bool>(3));
        assert!(row.get::<_, bool>(4));
        client
            .execute(
                "DELETE FROM passkey_challenges WHERE attempt_hash = $1",
                &[&hash],
            )
            .await
            .unwrap();
    }
}
