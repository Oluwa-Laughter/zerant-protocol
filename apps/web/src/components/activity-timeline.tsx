"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
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

export function mergeActivityEvents(current: ActivityEvent[], fresh: ActivityEvent[]): ActivityEvent[] {
  const freshIds = new Set(fresh.map((item) => item.id));
  return [...fresh, ...current.filter((item) => !freshIds.has(item.id))];
}

type ActivityCategory = "all" | "credentials" | "verification" | "security" | "zcash" | "other";

function eventCategory(type: string): Exclude<ActivityCategory, "all"> {
  if (type.startsWith("credential_")) return "credentials";
  if (type.startsWith("verification_")) return "verification";
  if (type.startsWith("zcash_")) return "zcash";
  if (type.includes("key") || type.includes("passkey") || type.includes("session") || type.includes("webhook")) return "security";
  return "other";
}

function eventTitle(type: string): string {
  switch (type) {
    case "credential_issued": return "Credential issued";
    case "credential_revoked": return "Credential revoked";
    case "credential_schema_created": return "Credential type created";
    case "credential_schema_versioned": return "Credential type updated";
    case "credential_schema_retired": return "Credential type retired";
    case "verification_requested": return "Verification requested";
    case "verification_approved": return "Verification approved";
    case "verification_denied": return "Verification denied";
    case "verification_expired": return "Verification expired";
    case "verifier_api_key_created": return "Integration key created";
    case "verifier_api_key_revoked": return "Integration key revoked";
    case "verifier_key_rotated": return "Verifier key rotated";
    case "issuer_key_rotated": return "Issuer key rotated";
    case "issuer_key_compromised": return "Issuer key replaced";
    case "passkey_attached": return "Passkey added";
    case "passkey_removed": return "Passkey removed";
    case "session_revoked": return "Session revoked";
    case "zcash_payment_prepared": return "Zcash payment prepared";
    case "zcash_payment_submitted": return "Zcash payment submitted";
    case "zcash_invoice_created": return "Zcash invoice created";
    case "zcash_invoice_cancelled": return "Zcash invoice cancelled";
    case "zcash_payout_shared": return "Private payout destination shared";
    case "zcash_payout_withdrawn": return "Private payout sharing withdrawn";
    default: return "Trust activity";
  }
}

function categoryLabel(category: ActivityCategory): string {
  switch (category) {
    case "all": return "All";
    case "credentials": return "Credentials";
    case "verification": return "Verification";
    case "security": return "Access & security";
    case "zcash": return "Zcash";
    case "other": return "Other";
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
  const [filter, setFilter] = useState<ActivityCategory>("all");
  const [refreshing, setRefreshing] = useState(false);
  const refreshInFlight = useRef(false);

  const categoryCounts = useMemo(() => {
    const counts: Record<Exclude<ActivityCategory, "all">, number> = { credentials: 0, verification: 0, security: 0, zcash: 0, other: 0 };
    for (const item of items) counts[eventCategory(item.event_type)] += 1;
    return counts;
  }, [items]);

  const visibleItems = useMemo(() =>
    filter === "all" ? items : items.filter((item) => eventCategory(item.event_type) === filter),
  [filter, items]);

  const availableCategories = useMemo(() =>
    (["credentials", "verification", "security", "zcash", "other"] as const).filter((category) => categoryCounts[category] > 0),
  [categoryCounts]);

  const refreshActivity = useCallback(async (announce = false) => {
    if (refreshInFlight.current) return;
    refreshInFlight.current = true;
    setRefreshing(true);
    try {
      const response = await fetch("/api/zerant/activity?limit=20", {
        credentials: "same-origin",
        cache: "no-store",
      });
      if (!response.ok) {
        if (announce) setStatus("Activity could not be refreshed right now.");
        return;
      }
      const page = (await response.json()) as ActivityPageData;
      setItems((current) => mergeActivityEvents(current, page.items));
      if (!cursor) setCursor(page.next_cursor);
      if (announce) setStatus("Activity refreshed.");
    } catch {
      if (announce) setStatus("Activity could not be refreshed right now.");
    } finally {
      refreshInFlight.current = false;
      setRefreshing(false);
    }
  }, [cursor]);

  useEffect(() => {
    const onFocus = () => { void refreshActivity(false); };
    const onVisibility = () => {
      if (document.visibilityState === "visible") void refreshActivity(false);
    };
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [refreshActivity]);

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
          See credential, verification, Zcash payment and developer-access changes associated with your Zerant account.
        </p>
      </section>

      {items.length ? (
        <section className="activity-toolbar" aria-label="Filter activity">
          <div>
            <span className="eyebrow">Recorded events</span>
            <strong>{items.length}</strong>
          </div>
          <div className="activity-filter-row" role="group" aria-label="Activity categories">
            <Button variant="secondary" disabled={refreshing} onClick={() => void refreshActivity(true)}>{refreshing ? "Refreshing…" : "Refresh"}</Button>
            <button type="button" className={filter === "all" ? "activity-filter active" : "activity-filter"} onClick={() => setFilter("all")}>All <span>{items.length}</span></button>
            {availableCategories.map((category) => (
              <button type="button" key={category} className={filter === category ? "activity-filter active" : "activity-filter"} onClick={() => setFilter(category)}>{categoryLabel(category)} <span>{categoryCounts[category]}</span></button>
            ))}
          </div>
        </section>
      ) : null}

      <section className="activity-list" aria-label="Zerant activity history">
        {visibleItems.length ? (
          visibleItems.map((item) => (
            <article className={`activity-card category-${eventCategory(item.event_type)}`} key={item.id}>
              <span className="activity-rail-dot" aria-hidden="true" />
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
                <span>{categoryLabel(eventCategory(item.event_type))}</span>
                {item.context ? <span>{item.context}</span> : null}
                {item.counterparty ? <span>{item.counterparty}</span> : null}
              </div>
            </article>
          ))
        ) : (
          <div className="activity-empty">
            <h2>{items.length ? "No events in this category." : "No activity yet."}</h2>
            <p className="muted">
              {items.length ? "Choose another filter to review the rest of your Zerant activity." : "Issuance, verification decisions, Zcash payment lifecycle and integration access changes will appear here."}
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
