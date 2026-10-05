"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Link from "next/link";
import { Button } from "@/components/ui/button";

export type VerifierProfile = {
  display_name: string;
  origin: string;
  created_at: string;
};

export type TrustedCredentialSchema = {
  id: string;
  issuer_id: string;
  issuer_name: string;
  display_name: string;
  description: string;
  claim_type: string;
  context: string;
  default_expiry_days: number;
  version: number;
  active: boolean;
  supersedes_schema_id: string | null;
  retired_at: string | null;
  created_at: string;
};

export type TrustedIssuerOption = {
  display_name: string;
  issuer_id: string;
  schemas: TrustedCredentialSchema[];
};



export type VerifierKeyView = {
  active: boolean;
  compromised: boolean;
  valid_from: string;
  retired_at: string | null;
};

export type VerifierApiKeyView = {
  id: string;
  name: string;
  key_prefix: string;
  scopes: string[];
  created_at: string;
  last_used_at: string | null;
  expires_at: string | null;
  revoked: boolean;
};

type CreatedVerifierApiKey = {
  key: VerifierApiKeyView;
  secret: string;
};

export type VerifierWebhookView = {
  id: string;
  name: string;
  url: string;
  created_at: string;
  last_delivery_at: string | null;
  disabled: boolean;
  pending_deliveries: number;
  dead_deliveries: number;
};

type CreatedVerifierWebhook = {
  webhook: VerifierWebhookView;
  secret: string;
};

export type VerificationRequestItem = {
  id: string;
  holder_zerant_id: string;
  purpose: string;
  credential_schema_id: string | null;
  credential_name: string | null;
  claim_type: string;
  context: string;
  status: string;
  verified: boolean;
  created_at: string;
  expires_at: string;
};

type VerifierHumanProof = {
  request_id: string;
  verifier_origin: string;
  credential_name: string | null;
  credential_version: number | null;
  issuer_id: string;
  claim_type: string;
  value: string | boolean | number;
  context: string | null;
  decided_at: string;
  proof_expires_at: string;
};

type VerificationRequestReview = {
  holderId: string;
  purpose: string;
  schemaId: string;
  schemaName: string;
  issuerId: string;
  issuerName: string;
  context: string;
};

export function VerifierWorkspace({
  authenticated,
  backendAvailable,
  initialProfile,
  issuers,
  initialRequests,
  initialKeys,
  initialApiKeys,
  initialWebhooks,
}: {
  authenticated: boolean;
  backendAvailable: boolean;
  initialProfile: VerifierProfile | null;
  issuers: TrustedIssuerOption[];
  initialRequests: VerificationRequestItem[];
  initialKeys: VerifierKeyView[];
  initialApiKeys: VerifierApiKeyView[];
  initialWebhooks: VerifierWebhookView[];
}) {
  const availableSchemas = useMemo(
    () =>
      issuers.flatMap((issuer) =>
        issuer.schemas
          .filter((schema) => schema.active)
          .map((schema) => ({
            ...schema,
            issuer_name: issuer.display_name,
            issuer_id: issuer.issuer_id,
          })),
      ),
    [issuers],
  );

  const [profile, setProfile] = useState(initialProfile);
  const [requests, setRequests] = useState(initialRequests);
  const [keys, setKeys] = useState<VerifierKeyView[]>(initialKeys);
  const [apiKeys, setApiKeys] = useState<VerifierApiKeyView[]>(initialApiKeys);
  const [webhooks, setWebhooks] =
    useState<VerifierWebhookView[]>(initialWebhooks);
  const [webhookName, setWebhookName] = useState("");
  const [webhookUrl, setWebhookUrl] = useState("");
  const [newWebhookSecret, setNewWebhookSecret] = useState<string | null>(null);
  const [apiKeyName, setApiKeyName] = useState("");
  const [apiKeyCreateScope, setApiKeyCreateScope] = useState(true);
  const [apiKeyReadScope, setApiKeyReadScope] = useState(true);
  const [apiKeyProofScope, setApiKeyProofScope] = useState(false);
  const [apiKeyExpiry, setApiKeyExpiry] = useState("90");
  const [newApiSecret, setNewApiSecret] = useState<string | null>(null);
  const [displayName, setDisplayName] = useState("");
  const [origin, setOrigin] = useState("");
  const [holderId, setHolderId] = useState("");
  const [purpose, setPurpose] = useState("");
  const [schemaId, setSchemaId] = useState(availableSchemas[0]?.id ?? "");
  const [requestReview, setRequestReview] = useState<VerificationRequestReview | null>(null);
  const [status, setStatus] = useState("");
  const [refreshingRequests, setRefreshingRequests] = useState(false);
  const requestsRefreshInFlight = useRef(false);
  const [proofResult, setProofResult] = useState<VerifierHumanProof | null>(null);
  const [proofLoadingId, setProofLoadingId] = useState<string | null>(null);
  const [retireConfirm, setRetireConfirm] = useState("");
  const [retiring, setRetiring] = useState(false);
  const [retired, setRetired] = useState(false);

  const selectedSchema = availableSchemas.find((schema) => schema.id === schemaId);
  const pendingRequestCount = requests.filter((request) => request.status === "pending").length;
  const approvedRequestCount = requests.filter((request) => request.status === "approved" && request.verified).length;
  const deniedOrExpiredCount = requests.filter((request) => request.status === "denied" || request.status === "expired").length;

  const refreshRequests = useCallback(async (announce = false) => {
    if (!authenticated || !profile || requestsRefreshInFlight.current) return;
    requestsRefreshInFlight.current = true;
    setRefreshingRequests(true);
    try {
      const response = await fetch("/api/zerant/verifier/requests", {
        credentials: "same-origin",
        cache: "no-store",
      });
      if (!response.ok) {
        if (announce) setStatus("Verification requests could not be refreshed right now.");
        return;
      }
      const nextRequests = (await response.json()) as VerificationRequestItem[];
      setRequests(nextRequests);
      setProofResult((current) =>
        current && nextRequests.some((request) => request.id === current.request_id && request.verified)
          ? current
          : null,
      );
      if (announce) setStatus("Verification requests refreshed.");
    } catch {
      if (announce) setStatus("Verification requests could not be refreshed right now.");
    } finally {
      requestsRefreshInFlight.current = false;
      setRefreshingRequests(false);
    }
  }, [authenticated, profile]);

  useEffect(() => {
    if (!authenticated || !profile) return;
    const onFocus = () => { void refreshRequests(false); };
    const onVisibility = () => {
      if (document.visibilityState === "visible") void refreshRequests(false);
    };
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [authenticated, profile, refreshRequests]);

  async function retireVerifier() {
    if (retiring) return;
    if (retireConfirm !== "RETIRE") {
      setStatus("Type RETIRE exactly before retiring this verifier profile.");
      return;
    }
    setRetiring(true);
    try {
      const response = await fetch("/api/zerant/verifier", {
        method: "DELETE",
        credentials: "same-origin",
      });
      if (!response.ok) {
        setStatus(
          response.status === 403
            ? "Sign in again before retiring this verifier profile."
            : response.status === 409
              ? "Finish or wait for pending verification requests and webhook deliveries before retiring this verifier profile."
              : response.status === 404
                ? "This verifier profile is already retired."
                : "Verifier retirement could not be completed.",
        );
        return;
      }
      setProfile(null);
      setRetired(true);
      setStatus("Verifier profile retired. Your account can now refresh its deletion eligibility.");
    } catch {
      setStatus("Verifier retirement could not be completed.");
    } finally {
      setRetiring(false);
    }
  }

  async function activate() {
    const response = await fetch("/api/zerant/verifier", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ display_name: displayName, origin }),
    });
    if (!response.ok) {
      if (response.status === 429) {
        setStatus("You’re doing that too quickly. Try again in a minute.");
        return;
      }
      setStatus(
        response.status === 409
          ? "This website or account is already registered."
          : "Verifier profile could not be created. Use the exact https:// website origin.",
      );
      return;
    }
    setProfile((await response.json()) as VerifierProfile);
    setStatus("Verifier profile is active.");
  }

  async function rotateVerifierKey(compromiseCurrent: boolean) {
    const response = await fetch("/api/zerant/verifier/keys/rotate", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ compromise_current: compromiseCurrent }),
    });

    if (!response.ok) {
      if (response.status === 429) {
        setStatus("You’re doing that too quickly. Try again in a minute.");
        return;
      }
      setStatus("Verification security key could not be replaced.");
      return;
    }

    const created = (await response.json()) as VerifierKeyView;
    const now = new Date().toISOString();
    setKeys((current) => [
      created,
      ...current.map((item) =>
        item.active
          ? {
              ...item,
              active: false,
              compromised: compromiseCurrent || item.compromised,
              retired_at: now,
            }
          : item,
      ),
    ]);

    if (compromiseCurrent) {
      const requestsResponse = await fetch("/api/zerant/verifier/requests", {
        credentials: "same-origin",
        cache: "no-store",
      });
      if (requestsResponse.ok) {
        setRequests((await requestsResponse.json()) as VerificationRequestItem[]);
      }
    }

    setStatus(
      compromiseCurrent
        ? "Compromised key replaced. Pending requests signed by it were expired."
        : "Security key rotated. Existing short-lived requests can finish normally.",
    );
  }

  async function createApiKey() {
    const scopes = [
      apiKeyCreateScope ? "requests:create" : null,
      apiKeyReadScope ? "requests:read" : null,
      apiKeyProofScope ? "proofs:read" : null,
    ].filter((scope): scope is string => Boolean(scope));

    if (!apiKeyName.trim() || !scopes.length) {
      setStatus("Name the integration and choose at least one permission.");
      return;
    }

    const days = Number.parseInt(apiKeyExpiry, 10);
    if (!Number.isInteger(days) || days < 1 || days > 365) {
      setStatus("Choose a valid integration-key expiry.");
      return;
    }

    const response = await fetch("/api/zerant/verifier/api-keys", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        name: apiKeyName,
        scopes,
        expires_in_days: days,
      }),
    });

    if (!response.ok) {
      setStatus(
        response.status === 429
          ? "You’re doing that too quickly. Try again in a minute."
          : "Integration key could not be created.",
      );
      return;
    }

    const created = (await response.json()) as CreatedVerifierApiKey;
    setApiKeys((current) => [created.key, ...current]);
    setNewApiSecret(created.secret);
    setApiKeyName("");
    setStatus("Integration key created. Copy the secret now; Zerant will not show it again.");
  }

  async function revokeApiKey(id: string) {
    const response = await fetch(
      "/api/zerant/verifier/api-keys/" + encodeURIComponent(id) + "/revoke",
      { method: "POST", credentials: "same-origin" },
    );

    if (!response.ok) {
      setStatus("Integration key could not be revoked.");
      return;
    }

    const revoked = (await response.json()) as VerifierApiKeyView;
    setApiKeys((current) =>
      current.map((item) => (item.id === revoked.id ? revoked : item)),
    );
    setStatus("Integration key revoked.");
  }

  async function createWebhook() {
    if (!webhookName.trim() || !webhookUrl.trim()) {
      setStatus("Name the webhook and enter its HTTPS endpoint.");
      return;
    }

    const response = await fetch("/api/zerant/verifier/webhooks", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        name: webhookName,
        url: webhookUrl,
      }),
    });

    if (!response.ok) {
      setStatus(
        response.status === 429
          ? "You’re doing that too quickly. Try again in a minute."
          : response.status === 409
            ? "That webhook is already active or you reached the endpoint limit."
            : "Webhook could not be created. Use a public HTTPS endpoint without a query string.",
      );
      return;
    }

    const created = (await response.json()) as CreatedVerifierWebhook;
    setWebhooks((current) => [created.webhook, ...current]);
    setNewWebhookSecret(created.secret);
    setWebhookName("");
    setWebhookUrl("");
    setStatus("Webhook created. Copy its signing secret now; Zerant will not show it again.");
  }

  async function disableWebhook(id: string) {
    const response = await fetch(
      "/api/zerant/verifier/webhooks/" + encodeURIComponent(id) + "/disable",
      {
        method: "POST",
        credentials: "same-origin",
      },
    );

    if (!response.ok) {
      setStatus("Webhook could not be disabled.");
      return;
    }

    const disabled = (await response.json()) as VerifierWebhookView;
    setWebhooks((current) =>
      current.map((item) => (item.id === disabled.id ? disabled : item)),
    );
    setStatus("Webhook disabled. Pending deliveries for it will not be sent.");
  }

  function proofValueLabel(value: VerifierHumanProof["value"]): string {
    if (typeof value === "boolean") return value ? "Yes" : "No";
    return String(value);
  }

  async function loadProofResult(id: string) {
    if (proofLoadingId) return;
    if (proofResult?.request_id === id) {
      setProofResult(null);
      setStatus("Narrow result closed.");
      return;
    }
    setProofLoadingId(id);
    setStatus("Verifying the approved result…");
    try {
      const response = await fetch(
        "/api/zerant/verifier/requests/" + encodeURIComponent(id) + "/proof",
        { credentials: "same-origin", cache: "no-store" },
      );
      if (!response.ok) {
        setStatus(
          response.status === 409
            ? "This request does not have an approved proof result yet."
            : response.status === 404
              ? "This proof result does not belong to this verifier account."
              : "The approved proof could not be verified right now.",
        );
        setProofResult(null);
        return;
      }
      const result = (await response.json()) as VerifierHumanProof;
      setProofResult(result);
      setStatus("Approved result verified. Only the bounded claim is shown below.");
    } catch {
      setProofResult(null);
      setStatus("The approved proof could not be verified right now.");
    } finally {
      setProofLoadingId(null);
    }
  }

  function reviewRequest() {
    if (!selectedSchema || !holderId.trim() || !purpose.trim()) {
      setStatus("Choose a recipient, purpose, and trusted credential type first.");
      return;
    }
    setRequestReview({
      holderId: holderId.trim(),
      purpose: purpose.trim(),
      schemaId: selectedSchema.id,
      schemaName: selectedSchema.display_name,
      issuerId: selectedSchema.issuer_id,
      issuerName: selectedSchema.issuer_name,
      context: selectedSchema.context,
    });
    setStatus("Review the exact verification request before sending it.");
  }

  async function createRequest() {
    if (!requestReview) {
      setStatus("Review the request before sending it.");
      return;
    }

    const response = await fetch("/api/zerant/verifier/requests", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        holder_zerant_id: requestReview.holderId,
        purpose: requestReview.purpose,
        credential_schema_id: requestReview.schemaId,
        accepted_issuer_ids: [requestReview.issuerId],
      }),
    });
    if (!response.ok) {
      if (response.status === 429) {
        setStatus("You’re doing that too quickly. Try again in a minute.");
        return;
      }
      setStatus(
        response.status === 404
          ? "That Zerant ID or credential type could not be found."
          : "Verification request could not be created. Check the details and try again.",
      );
      return;
    }

    const created = (await response.json()) as VerificationRequestItem;
    setRequests((current) => [created, ...current]);
    setHolderId("");
    setPurpose("");
    setRequestReview(null);
    setStatus("Request sent. The recipient has five minutes to approve or deny it.");
  }

  if (!backendAvailable) {
    return (
      <main id="main" className="verifier-page">
        <section className="verifier-hero">
          <p className="eyebrow">Verify with Zerant</p>
          <h1>Verification is temporarily unavailable.</h1>
        </section>
      </main>
    );
  }

  if (!authenticated) {
    return (
      <main id="main" className="verifier-page">
        <section className="verifier-hero">
          <p className="eyebrow">Verify with Zerant</p>
          <h1>Ask for proof, not a person&apos;s entire profile.</h1>
          <p>
            Request a trusted credential from an accepted issuer without collecting unrelated
            personal information or wallet history.
          </p>
          <Link href="/vault" className="button">
            Connect to Zerant <span aria-hidden="true">→</span>
          </Link>
        </section>
      </main>
    );
  }

  if (retired) {
    return (
      <main id="main" className="verifier-page">
        <section className="verifier-hero">
          <p className="eyebrow">Verifier retired</p>
          <h1>This verifier profile has been removed.</h1>
          <p>Verifier keys, integrations, policies and verifier-side request records were removed. Holder-side Zerant activity remains as historical metadata.</p>
          <Link href="/account" className="button">Review account settings <span aria-hidden="true">→</span></Link>
          {status ? <p className="vault-status neutral" role="status">{status}</p> : null}
        </section>
      </main>
    );
  }

  if (!profile) {
    return (
      <main id="main" className="verifier-page">
        <section className="verifier-hero">
          <p className="eyebrow">Verify with Zerant</p>
          <h1>Connect your application to private trust.</h1>
          <p>
            Register the application or organization that will ask users for proof. Requests are
            tied to this website so a proof cannot be reused somewhere else.
          </p>
        </section>
        <section className="verifier-panel verifier-register">
          <label htmlFor="verifier-name">Application or organization name</label>
          <input
            id="verifier-name"
            value={displayName}
            onChange={(event) => setDisplayName(event.target.value)}
            placeholder="Your application name"
          />
          <label htmlFor="verifier-origin">Website</label>
          <input
            id="verifier-origin"
            value={origin}
            onChange={(event) => setOrigin(event.target.value)}
            placeholder="https://yourapp.com"
          />
          <Button disabled={!displayName.trim() || !origin.trim()} onClick={activate}>
            Activate verification
          </Button>
          {status ? <p className="vault-status neutral" role="status">{status}</p> : null}
        </section>
      </main>
    );
  }

  return (
    <main id="main" className="verifier-page">
      <section className="verifier-hero verifier-hero-row">
        <div>
          <p className="eyebrow">Verifier workspace</p>
          <h1>{profile.display_name}</h1>
          <p>Request only the trusted fact your application actually needs.</p>
        </div>
        <span className="pill">{profile.origin}</span>
      </section>

      <section className="verifier-progress" aria-label="Verifier request progress">
        <div className="verifier-progress-heading">
          <div><p className="eyebrow">Verification flow</p><h2>Ask for one fact. Receive one bounded result.</h2></div>
          <span className="small muted">Audience-bound · holder-approved</span>
        </div>
        <div className="verifier-progress-grid">
          <article className="verifier-progress-step complete">
            <span className="verifier-progress-number">01</span>
            <div><strong>Verifier profile</strong><span>{profile.display_name}</span></div>
            <span className="verifier-progress-state">Complete</span>
          </article>
          <a className={availableSchemas.length ? "verifier-progress-step complete" : "verifier-progress-step current"} href="#new-verification-request">
            <span className="verifier-progress-number">02</span>
            <div><strong>Choose trusted claim</strong><span>{availableSchemas.length ? `${availableSchemas.length} credential type${availableSchemas.length === 1 ? "" : "s"} available` : "Waiting for a trusted issuer credential type"}</span></div>
            <span className="verifier-progress-state">{availableSchemas.length ? "Ready" : "Next"}</span>
          </a>
          <a className={requests.length ? "verifier-progress-step complete" : availableSchemas.length ? "verifier-progress-step current" : "verifier-progress-step"} href="#new-verification-request">
            <span className="verifier-progress-number">03</span>
            <div><strong>Request proof</strong><span>{requests.length ? `${requests.length} request${requests.length === 1 ? "" : "s"} created` : "Recipient reviews the exact claim and purpose"}</span></div>
            <span className="verifier-progress-state">{requests.length ? "Active" : availableSchemas.length ? "Next" : "Waiting"}</span>
          </a>
          <a className={approvedRequestCount ? "verifier-progress-step complete" : pendingRequestCount ? "verifier-progress-step current" : "verifier-progress-step"} href="#verification-results">
            <span className="verifier-progress-number">04</span>
            <div><strong>Bounded result</strong><span>{approvedRequestCount ? `${approvedRequestCount} verified · ${pendingRequestCount} pending` : pendingRequestCount ? `${pendingRequestCount} waiting for holder decision` : deniedOrExpiredCount ? `${deniedOrExpiredCount} denied or expired` : "No result collected yet"}</span></div>
            <span className="verifier-progress-state">{approvedRequestCount ? "Ready" : pendingRequestCount ? "Waiting" : "Pending"}</span>
          </a>
        </div>
      </section>

      <section className="verifier-security-section">
        <article className="verifier-panel">
          <p className="eyebrow">Verification security</p>
          <h2>Keep request signing healthy.</h2>
          <p className="muted">
            Routine rotation changes the key used for new requests while already-sent requests keep
            their normal short expiry.
          </p>
          <div className="vault-actions wrap">
            <Button variant="secondary" onClick={() => rotateVerifierKey(false)}>
              Rotate security key
            </Button>
            <Button variant="secondary" onClick={() => rotateVerifierKey(true)}>
              Replace compromised key
            </Button>
          </div>
        </article>

        <article className="verifier-panel">
          <p className="eyebrow">Key history</p>
          <h2>{keys.length} key{keys.length === 1 ? "" : "s"}</h2>
          <div className="key-history-list">
            {keys.length ? (
              keys.map((key, index) => (
                <article className="key-history-card" key={key.valid_from + String(index)}>
                  <div>
                    <strong>{key.active ? "Current security key" : "Previous security key"}</strong>
                    <span
                      className={
                        key.compromised
                          ? "credential-status revoked"
                          : key.active
                            ? "credential-status active"
                            : "request-status"
                      }
                    >
                      {key.compromised ? "Compromised" : key.active ? "Active" : "Retired"}
                    </span>
                  </div>
                  <p className="small muted">
                    Active since {new Date(key.valid_from).toLocaleDateString()}
                    {key.retired_at
                      ? " · retired " + new Date(key.retired_at).toLocaleDateString()
                      : ""}
                  </p>
                </article>
              ))
            ) : (
              <p className="muted">Security-key history will appear here.</p>
            )}
          </div>
        </article>
      </section>

      <section className="verifier-integration-section">
        <article className="verifier-panel">
          <p className="eyebrow">Developer integration</p>
          <h2>Connect your server to Zerant.</h2>
          <p className="muted">
            Create a scoped key for your application backend. Integration keys are for
            server-to-server use only and should never be placed in browser code.
          </p>

          <label htmlFor="integration-key-name">Integration name</label>
          <input
            id="integration-key-name"
            value={apiKeyName}
            onChange={(event) => setApiKeyName(event.target.value)}
            placeholder="Production backend"
          />

          <fieldset className="integration-scope-options">
            <legend>Permissions</legend>
            <label>
              <input
                type="checkbox"
                checked={apiKeyCreateScope}
                onChange={(event) => setApiKeyCreateScope(event.target.checked)}
              />
              Create verification requests
            </label>
            <label>
              <input
                type="checkbox"
                checked={apiKeyReadScope}
                onChange={(event) => setApiKeyReadScope(event.target.checked)}
              />
              Read request status
            </label>
            <label>
              <input
                type="checkbox"
                checked={apiKeyProofScope}
                onChange={(event) => setApiKeyProofScope(event.target.checked)}
              />
              Read signed proof packages
            </label>
          </fieldset>

          <label htmlFor="integration-key-expiry">Expires in</label>
          <select
            id="integration-key-expiry"
            value={apiKeyExpiry}
            onChange={(event) => setApiKeyExpiry(event.target.value)}
          >
            <option value="30">30 days</option>
            <option value="90">90 days</option>
            <option value="180">180 days</option>
            <option value="365">1 year</option>
          </select>

          <Button disabled={!apiKeyName.trim()} onClick={createApiKey}>
            Create integration key
          </Button>

          {newApiSecret ? (
            <div className="integration-secret-card">
              <strong>Copy this secret now.</strong>
              <p className="small muted">Zerant stores only its hash and cannot show it again.</p>
              <code>{newApiSecret}</code>
              <Button
                variant="secondary"
                onClick={() => void navigator.clipboard?.writeText(newApiSecret)}
              >
                Copy secret
              </Button>
            </div>
          ) : null}

          <div className="integration-endpoints">
            <span className="eyebrow">Server endpoints</span>
            <code>POST /api/zerant/integrations/verifier/requests</code>
            <code>GET /api/zerant/integrations/verifier/requests/&lt;request-id&gt;</code>
            <code>GET /api/zerant/integrations/verifier/requests/&lt;request-id&gt;/proof</code>
          </div>
        </article>

        <article className="verifier-panel">
          <p className="eyebrow">Integration keys</p>
          <h2>{apiKeys.length} key{apiKeys.length === 1 ? "" : "s"}</h2>
          <div className="integration-key-list">
            {apiKeys.length ? (
              apiKeys.map((key) => (
                <article className="integration-key-card" key={key.id}>
                  <div className="integration-key-card-top">
                    <div>
                      <strong>{key.name}</strong>
                      <code>{key.key_prefix}…</code>
                    </div>
                    <span className={key.revoked ? "request-status denied" : "request-status approved"}>
                      {key.revoked ? "Revoked" : "Active"}
                    </span>
                  </div>
                  <p className="small muted">{key.scopes.join(" · ")}</p>
                  <p className="small muted">
                    Created {new Date(key.created_at).toLocaleDateString()}
                    {key.last_used_at ? " · last used " + new Date(key.last_used_at).toLocaleDateString() : " · never used"}
                    {key.expires_at ? " · expires " + new Date(key.expires_at).toLocaleDateString() : ""}
                  </p>
                  {!key.revoked ? (
                    <Button variant="secondary" onClick={() => revokeApiKey(key.id)}>
                      Revoke key
                    </Button>
                  ) : null}
                </article>
              ))
            ) : (
              <p className="muted">No integration keys yet.</p>
            )}
          </div>
        </article>
      </section>

      <section className="verifier-webhook-section">
        <article className="verifier-panel">
          <p className="eyebrow">Result webhooks</p>
          <h2>Receive verification results automatically.</h2>
          <p className="muted">
            Zerant can notify your server when a holder approves or denies a request. Delivery
            payloads contain request status only — never the holder’s credential contents or wallet
            information.
          </p>

          <label htmlFor="webhook-name">Endpoint name</label>
          <input
            id="webhook-name"
            value={webhookName}
            onChange={(event) => setWebhookName(event.target.value)}
            placeholder="Production results"
          />

          <label htmlFor="webhook-url">HTTPS endpoint</label>
          <input
            id="webhook-url"
            value={webhookUrl}
            onChange={(event) => setWebhookUrl(event.target.value)}
            placeholder="https://yourapp.com/api/zerant/webhook"
            spellCheck={false}
          />

          <Button disabled={!webhookName.trim() || !webhookUrl.trim()} onClick={createWebhook}>
            Add webhook
          </Button>

          {newWebhookSecret ? (
            <div className="integration-secret-card">
              <strong>Copy this webhook secret now.</strong>
              <p className="small muted">
                Use it to verify the signature on every Zerant delivery. It will not be shown
                again.
              </p>
              <code>{newWebhookSecret}</code>
              <Button
                variant="secondary"
                onClick={() => void navigator.clipboard?.writeText(newWebhookSecret)}
              >
                Copy webhook secret
              </Button>
            </div>
          ) : null}
        </article>

        <article className="verifier-panel">
          <p className="eyebrow">Webhook health</p>
          <h2>{webhooks.length} endpoint{webhooks.length === 1 ? "" : "s"}</h2>
          <div className="webhook-list">
            {webhooks.length ? (
              webhooks.map((webhook) => (
                <article className="webhook-card" key={webhook.id}>
                  <div className="webhook-card-top">
                    <div>
                      <strong>{webhook.name}</strong>
                      <code>{webhook.url}</code>
                    </div>
                    <span
                      className={webhook.disabled ? "request-status denied" : "request-status approved"}
                    >
                      {webhook.disabled ? "Disabled" : "Active"}
                    </span>
                  </div>
                  <p className="small muted">
                    {webhook.last_delivery_at
                      ? "Last delivered " + new Date(webhook.last_delivery_at).toLocaleString()
                      : "No successful delivery yet"}
                  </p>
                  <div className="webhook-health">
                    <span>{webhook.pending_deliveries} pending</span>
                    <span>{webhook.dead_deliveries} failed permanently</span>
                  </div>
                  {!webhook.disabled ? (
                    <Button variant="secondary" onClick={() => disableWebhook(webhook.id)}>
                      Disable webhook
                    </Button>
                  ) : null}
                </article>
              ))
            ) : (
              <p className="muted">No result webhooks configured yet.</p>
            )}
          </div>
        </article>
      </section>

      <section className="verifier-grid" id="new-verification-request">
        <article className="verifier-panel">
          <p className="eyebrow">New request</p>
          <h2>What do you need to verify?</h2>

          <label htmlFor="verify-holder">Recipient Zerant ID</label>
          <input
            id="verify-holder"
            value={holderId}
            onChange={(event) => { setHolderId(event.target.value); setRequestReview(null); }}
            placeholder="zr_..."
          />

          <label htmlFor="verify-purpose">Why do you need this?</label>
          <textarea
            id="verify-purpose"
            value={purpose}
            onChange={(event) => { setPurpose(event.target.value); setRequestReview(null); }}
            rows={4}
            placeholder="Explain the decision this proof will be used for."
          />

          <label htmlFor="verify-schema">Trusted credential type</label>
          <select
            id="verify-schema"
            value={schemaId}
            onChange={(event) => { setSchemaId(event.target.value); setRequestReview(null); }}
          >
            {availableSchemas.length ? (
              availableSchemas.map((schema) => (
                <option value={schema.id} key={schema.id}>
                  {schema.display_name} v{schema.version} · {schema.issuer_name}
                </option>
              ))
            ) : (
              <option value="">No trusted credential types available yet</option>
            )}
          </select>

          {selectedSchema ? (
            <div className="selected-schema-summary">
              <strong>{selectedSchema.display_name}</strong>
              <p>{selectedSchema.description}</p>
              <span className="small muted">
                Issued by {selectedSchema.issuer_name} · {selectedSchema.context}
              </span>
            </div>
          ) : null}

          <p className="small muted">
            Requests are short-lived. The recipient has five minutes to review the exact claim and respond.
          </p>

          {requestReview ? (
            <div className="verifier-request-review" aria-label="Review verification request before sending">
              <div className="verifier-request-review-heading">
                <div><span className="eyebrow">Final review</span><h3>Send only what you actually need.</h3></div>
                <span className="request-status">5 min</span>
              </div>
              <dl>
                <div><dt>Recipient</dt><dd className="mono">{requestReview.holderId}</dd></div>
                <div><dt>Credential</dt><dd>{requestReview.schemaName}</dd></div>
                <div><dt>Trusted issuer</dt><dd>{requestReview.issuerName}</dd></div>
                <div><dt>Context</dt><dd>{requestReview.context}</dd></div>
                <div className="verifier-review-purpose"><dt>Purpose shown to holder</dt><dd>{requestReview.purpose}</dd></div>
              </dl>
              <p className="small muted">The holder will see who is asking, this purpose, and the exact claim Zerant proposes to share before they can approve.</p>
              <div className="vault-actions wrap">
                <Button onClick={() => void createRequest()}>Send verification request</Button>
                <Button variant="secondary" onClick={() => { setRequestReview(null); setStatus("Edit the request details, then review again."); }}>Edit details</Button>
              </div>
            </div>
          ) : (
            <Button
              disabled={!holderId.trim() || !purpose.trim() || !selectedSchema}
              onClick={reviewRequest}
            >
              Review request
            </Button>
          )}
          {status ? <p className="vault-status neutral" role="status">{status}</p> : null}
        </article>

        <article className="verifier-panel" id="verification-results">
          <div className="verifier-request-list-heading">
            <div><p className="eyebrow">Requests</p><h2>{requests.length} request{requests.length === 1 ? "" : "s"}</h2></div>
            <Button variant="secondary" disabled={refreshingRequests} onClick={() => void refreshRequests(true)}>{refreshingRequests ? "Refreshing…" : "Refresh requests"}</Button>
          </div>
          <div className="verification-list">
            {requests.length ? (
              requests.map((request) => (
                <article className="verification-card" key={request.id}>
                  <div className="verification-card-top">
                    <strong>{request.credential_name ?? "Legacy credential"}</strong>
                    <span className={"request-status " + request.status}>
                      {request.verified ? "Verified" : request.status}
                    </span>
                  </div>
                  <p>{request.purpose}</p>
                  <p className="small muted">
                    {request.context} · {request.holder_zerant_id}
                  </p>
                  <p className="small muted">Created {new Date(request.created_at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })} · expires {new Date(request.expires_at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</p>
                  {request.verified ? (
                    <div className="verification-result-actions">
                      <Button variant="secondary" disabled={proofLoadingId !== null} onClick={() => void loadProofResult(request.id)}>
                        {proofLoadingId === request.id ? "Verifying…" : proofResult?.request_id === request.id ? "Close narrow result" : "View narrow result"}
                      </Button>
                    </div>
                  ) : null}
                  {proofResult?.request_id === request.id ? (
                    <div className="verification-result-card" aria-label="Verified narrow result">
                      <div className="verification-result-heading">
                        <div><span className="eyebrow">Verified result</span><h3>{proofResult.credential_name ?? request.credential_name ?? "Trusted claim"}{proofResult.credential_version ? ` v${proofResult.credential_version}` : ""}</h3></div>
                        <span className="request-status approved">Verified</span>
                      </div>
                      <div className="verification-result-value"><span className="eyebrow">Bounded claim</span><strong>{proofValueLabel(proofResult.value)}</strong></div>
                      <dl>
                        <div><dt>Claim type</dt><dd>{proofResult.claim_type}</dd></div>
                        <div><dt>Context</dt><dd>{proofResult.context ?? request.context}</dd></div>
                        <div><dt>Trusted issuer</dt><dd className="mono">{proofResult.issuer_id}</dd></div>
                        <div><dt>Verified for</dt><dd>{proofResult.verifier_origin}</dd></div>
                        <div><dt>Holder decided</dt><dd>{new Date(proofResult.decided_at).toLocaleString()}</dd></div>
                        <div><dt>Proof expires</dt><dd>{new Date(proofResult.proof_expires_at).toLocaleString()}</dd></div>
                      </dl>
                      <p className="small muted">This view is produced only after Zerant re-verifies the signed request, holder response, issuer key, verifier key and revocation evidence. The holder’s full credential, wallet address, balance and wallet history are not included.</p>
                    </div>
                  ) : null}
                </article>
              ))
            ) : (
              <p className="muted">No verification requests yet.</p>
            )}
          </div>
        </article>
      </section>

      <section className="verifier-retirement" aria-labelledby="verifier-retirement-title">
        <div>
          <p className="eyebrow">Verifier lifecycle</p>
          <h2 id="verifier-retirement-title">Retire this verifier profile.</h2>
          <p className="muted">Retirement permanently removes this verifier’s signing keys, integration keys, webhooks, policies and verifier-side request records. Holder-side Zerant activity remains as historical metadata. Pending requests or webhook deliveries must finish first.</p>
        </div>
        <label htmlFor="verifier-retire-confirm">Type RETIRE to confirm</label>
        <input id="verifier-retire-confirm" value={retireConfirm} onChange={(event) => setRetireConfirm(event.target.value)} autoComplete="off" />
        <Button variant="secondary" disabled={retireConfirm !== "RETIRE" || retiring} onClick={() => void retireVerifier()}>
          {retiring ? "Retiring…" : "Retire verifier profile"}
        </Button>
      </section>
    </main>
  );
}
