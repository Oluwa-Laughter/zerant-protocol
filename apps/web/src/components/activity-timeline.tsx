"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";

export type ActivityEvent = {
  id: number;
  event_type: string;
  object_id: string;
  label: string;
  context: string | null;
  counterparty: string | null;
  created_at: string;
};

export type ActivityPageData = {
  items: ActivityEvent[];
  next_cursor: string | null;
};

function eventTitle(type: string): string {
  switch (type) {
    case "credential_issued":
      return "Credential issued";
    case "credential_revoked":
      return "Credential revoked";
    case "verification_requested":
      return "Verification requested";
    case "verification_approved":
      return "Verification approved";
    case "verification_denied":
      return "Verification denied";
    case "verification_expired":
      return "Verification expired";
    case "verifier_api_key_created":
      return "Integration key created";
    case "verifier_api_key_revoked":
      return "Integration key revoked";
    default:
      return "Trust activity";
  }
}

export function ActivityTimeline({
  initialPage,
}: {
  initialPage: ActivityPageData;
}) {
  const [items, setItems] = useState(initialPage.items);
  const [cursor, setCursor] = useState(initialPage.next_cursor);
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState("");

  async function loadMore() {
    if (!cursor || loading) return;
    setLoading(true);
    const response = await fetch(
      "/api/zerant/activity?limit=20&cursor=" + encodeURIComponent(cursor),
      { credentials: "same-origin", cache: "no-store" },
    );
    if (!response.ok) {
      setStatus("More activity could not be loaded.");
      setLoading(false);
      return;
    }

    const page = (await response.json()) as ActivityPageData;
    setItems((current) => [...current, ...page.items]);
    setCursor(page.next_cursor);
    setStatus("");
    setLoading(false);
  }

  return (
    <main id="main" className="activity-page">
      <section className="activity-hero">
        <p className="eyebrow">Activity</p>
        <h1>Your trust history.</h1>
        <p>
          See credential, verification and developer-access changes associated with your Zerant account.
        </p>
      </section>

      <section className="activity-list" aria-label="Zerant activity history">
        {items.length ? (
          items.map((item) => (
            <article className="activity-card" key={item.id}>
              <div className="activity-card-top">
                <div>
                  <span className="eyebrow">{eventTitle(item.event_type)}</span>
                  <h2>{item.label}</h2>
                </div>
                <time dateTime={item.created_at}>
                  {new Date(item.created_at).toLocaleString()}
                </time>
              </div>
              <div className="activity-meta">
                {item.context ? <span>{item.context}</span> : null}
                {item.counterparty ? <span>{item.counterparty}</span> : null}
              </div>
            </article>
          ))
        ) : (
          <div className="activity-empty">
            <h2>No activity yet.</h2>
            <p className="muted">
              Issuance, verification decisions and integration access changes will appear here.
            </p>
          </div>
        )}
      </section>

      {cursor ? (
        <div className="activity-more">
          <Button variant="secondary" disabled={loading} onClick={loadMore}>
            {loading ? "Loading…" : "Load more"}
          </Button>
        </div>
      ) : null}

      {status ? <p className="vault-status neutral" role="status">{status}</p> : null}
    </main>
  );
}
