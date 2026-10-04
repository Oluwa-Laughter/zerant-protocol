"use client";

import { useState } from "react";
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

  async function decide(id: string, decision: "approve" | "deny") {
    const response = await fetch(
      "/api/zerant/holder/requests/" + encodeURIComponent(id) + "/decision",
      {
        method: "POST",
        credentials: "same-origin",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ decision }),
      },
    );
    if (!response.ok) {
      if (response.status === 429) { setStatus("You’re doing that too quickly. Try again in a minute."); return; }
      setStatus(
        response.status === 404
          ? "A matching private credential could not be found for this request."
          : "This request could not be completed. It may have expired.",
      );
      return;
    }

    setRequests((current) => current.filter((item) => item.id !== id));
    setStatus(
      decision === "approve"
        ? "Approved. Zerant sent only the requested proof."
        : "Request denied. No credential information was shared.",
    );
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
      <section className="requests-hero">
        <p className="eyebrow">Verification requests</p>
        <h1>Review before you prove.</h1>
        <p>
          Every request shows who is asking, what they want to verify, why they need it, and where
          the result will be used.
        </p>
      </section>

      <section className="holder-request-list">
        {requests.length ? requests.map((request) => (
          <article className="holder-request-card" key={request.id}>
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
              <p><strong>If you approve:</strong> Zerant will return only a proof for this request.</p>
              <p><strong>It will not share:</strong> your complete credential, other credentials, payment address, balance or wallet history.</p>
            </div>

            <p className="small muted">
              Requested by {request.verifier_origin} · expires{" "}
              {new Date(request.expires_at).toLocaleTimeString([], {
                hour: "2-digit",
                minute: "2-digit",
              })}
            </p>

            <div className="vault-actions">
              <Button onClick={() => decide(request.id, "approve")}>Approve proof</Button>
              <Button variant="secondary" onClick={() => decide(request.id, "deny")}>Deny</Button>
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
