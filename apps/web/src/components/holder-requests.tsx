"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import Link from "next/link";
import { Button } from "@/components/ui/button";

export type HolderVerificationRequest = {
  id: string;
  verifier_name: string;
  verifier_origin: string;
  purpose: string;
  credential_name: string | null;
  claim_type: string;
  context: string;
  created_at: string;
  expires_at: string;
};

type ProofPreview = {
  request: HolderVerificationRequest;
  issuer_id: string;
  issuer_name: string;
  value: string;
};

export function HolderRequests({
  authenticated,
  backendAvailable,
  initialRequests,
}: {
  authenticated: boolean;
  backendAvailable: boolean;
  initialRequests: HolderVerificationRequest[];
}) {
  const [requests, setRequests] = useState(initialRequests);
  const [status, setStatus] = useState("");
  const [preview, setPreview] = useState<ProofPreview | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const refreshInFlight = useRef(false);

  const refreshRequests = useCallback(async (announce = false) => {
    if (!authenticated || !backendAvailable || refreshInFlight.current) return;
    refreshInFlight.current = true;
    setRefreshing(true);
    try {
      const response = await fetch("/api/zerant/holder/requests", {
        credentials: "same-origin",
        cache: "no-store",
      });
      if (!response.ok) {
        if (announce) setStatus("Requests could not be refreshed right now.");
        return;
      }
      const nextRequests = await response.json() as HolderVerificationRequest[];
      setRequests(nextRequests);
      setPreview((current) =>
        current && nextRequests.some((item) => item.id === current.request.id) ? current : null,
      );
      if (announce) setStatus("Requests refreshed.");
    } catch {
      if (announce) setStatus("Requests could not be refreshed right now.");
    } finally {
      refreshInFlight.current = false;
      setRefreshing(false);
    }
  }, [authenticated, backendAvailable]);

  useEffect(() => {
    if (!authenticated || !backendAvailable) return;
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
  }, [authenticated, backendAvailable, refreshRequests]);

  async function review(id: string) {
    if (busyId) return;
    setBusyId(id);
    setStatus("");
    try {
      const response = await fetch(
        "/api/zerant/holder/requests/" + encodeURIComponent(id) + "/preview",
        { credentials: "same-origin", cache: "no-store" },
      );
      if (!response.ok) {
        setStatus(response.status === 404
          ? "This request has expired, or no matching private credential is available."
          : "The proof could not be reviewed right now.");
        return;
      }
      setPreview(await response.json() as ProofPreview);
      setStatus("Proof reviewed. Nothing is shared until you approve the exact claim below.");
    } catch {
      setStatus("The proof could not be reviewed right now.");
    } finally {
      setBusyId(null);
    }
  }

  async function decide(id: string, decision: "approve" | "deny") {
    if (busyId || (decision === "approve" && preview?.request.id !== id)) return;
    setBusyId(id);
    setStatus("");
    try {
      const response = await fetch(
        "/api/zerant/holder/requests/" + encodeURIComponent(id) + "/decision",
        {
          method: "POST",
          credentials: "same-origin",
          headers: { "content-type": "application/json" },
          body: JSON.stringify(decision === "approve" ? {
            decision,
            expected_issuer_id: preview?.issuer_id,
            expected_value: preview?.value,
          } : { decision }),
        },
      );
      if (!response.ok) {
        if (response.status === 429) {
          setStatus("You’re doing that too quickly. Try again in a minute.");
          return;
        }
        setPreview(null);
        setStatus(
          response.status === 404
            ? "A matching private credential could not be found for this request."
            : "The request or available credential changed. Review it again if it is still waiting.",
        );
        return;
      }

      setRequests((current) => current.filter((item) => item.id !== id));
      setPreview(null);
      setStatus(
        decision === "approve"
          ? "Approved. The claim you reviewed is now available to this verifier."
          : "Request denied. No credential information was shared.",
      );
    } catch {
      setStatus("This request could not be completed. Try again.");
    } finally {
      setBusyId(null);
    }
  }

  if (!backendAvailable) {
    return (
      <main id="main" className="requests-page">
        <section className="requests-hero">
          <p className="eyebrow">Verification requests</p>
          <h1>Requests are temporarily unavailable.</h1>
        </section>
      </main>
    );
  }

  if (!authenticated) {
    return (
      <main id="main" className="requests-page">
        <section className="requests-hero">
          <p className="eyebrow">Verification requests</p>
          <h1>You decide what others can verify.</h1>
          <p>Connect to Zerant to review requests made against your private credentials.</p>
          <Link href="/vault" className="button">Connect to Zerant <span aria-hidden="true">→</span></Link>
        </section>
      </main>
    );
  }

  return (
    <main id="main" className="requests-page">
      <section className="requests-hero holder-requests-hero">
        <div>
          <p className="eyebrow">Verification requests</p>
          <h1>Review before you prove.</h1>
          <p>
            Every request shows who is asking, what they want to verify, why they need it, and where
            the result will be used.
          </p>
        </div>
        <div className="holder-requests-actions">
          <span className="pill">{requests.length} waiting</span>
          <Button variant="secondary" disabled={refreshing || busyId !== null} onClick={() => void refreshRequests(true)}>
            {refreshing ? "Refreshing…" : "Refresh requests"}
          </Button>
        </div>
      </section>

      <section className="holder-request-list">
        {requests.length ? requests.map((request) => (
          <article className={preview?.request.id === request.id ? "holder-request-card is-reviewing" : "holder-request-card"} key={request.id}>
            <div className="holder-request-heading">
              <div>
                <span className="eyebrow">{request.verifier_name}</span>
                <h2>{request.credential_name ?? "Legacy credential"}</h2>
              </div>
              <span className="pill">{request.context}</span>
            </div>

            <div className="request-purpose">
              <span className="eyebrow">Why they need it</span>
              <p>{request.purpose}</p>
            </div>

            <div className="request-disclosure">
              <p><strong>If you approve:</strong> Zerant will share the exact claim shown in your review with this requester.</p>
              <p><strong>It will not share:</strong> your complete credential, other credentials, payment address, balance or wallet history.</p>
            </div>

            <div className="holder-request-meta">
              <span className="small muted">Requested by {request.verifier_origin}</span>
              <span className="request-expiry-pill">Expires {new Date(request.expires_at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</span>
            </div>

            {preview?.request.id === request.id ? (
              <div className="holder-proof-preview" aria-label="Exact claim to be shared">
                <div className="holder-proof-preview-heading">
                  <div><span className="eyebrow">Exact disclosure</span><h3>This is the claim Zerant would share.</h3></div>
                  <span className="request-status approved">Reviewed</span>
                </div>
                <div className="holder-proof-value"><span className="eyebrow">Claim</span><strong>{preview.value}</strong></div>
                <dl>
                  <div><dt>Issued by</dt><dd>{preview.issuer_name}</dd></div>
                  <div><dt>Requester</dt><dd>{preview.request.verifier_name}</dd></div>
                  <div><dt>Used at</dt><dd>{preview.request.verifier_origin}</dd></div>
                  <div><dt>Purpose</dt><dd>{preview.request.purpose}</dd></div>
                </dl>
                <p className="small muted">Your complete credential, other credentials, payment address, balance and wallet history stay out of this response.</p>
              </div>
            ) : null}
            <div className="vault-actions wrap">
              {preview?.request.id === request.id ? (
                <>
                  <Button disabled={busyId !== null} onClick={() => decide(request.id, "approve")}>Approve this claim</Button>
                  <Button variant="secondary" disabled={busyId !== null} onClick={() => { setPreview(null); setStatus("Review closed. Nothing was shared."); }}>Back</Button>
                </>
              ) : (
                <Button disabled={busyId !== null} onClick={() => review(request.id)}>Review exact claim</Button>
              )}
              <Button variant="secondary" disabled={busyId !== null} onClick={() => decide(request.id, "deny")}>Deny request</Button>
            </div>
          </article>
        )) : (
          <div className="requests-empty">
            <h2>No requests waiting.</h2>
            <p className="muted">When an application asks you to prove something, it will appear here.</p>
          </div>
        )}
      </section>

      {status ? <p className="vault-status neutral" role="status">{status}</p> : null}
    </main>
  );
}
