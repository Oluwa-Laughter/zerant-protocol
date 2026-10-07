"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import type { WorkspaceZcashNetwork } from "@/components/product-workspace";

export function ZcashLiveStatus({
  authenticated,
  network,
  onNetworkUpdate,
}: {
  authenticated: boolean;
  network: WorkspaceZcashNetwork;
  onNetworkUpdate?: (network: WorkspaceZcashNetwork) => void;
}) {
  const [refreshedNetwork, setRefreshedNetwork] = useState<WorkspaceZcashNetwork>(null);
  const [refreshingNetwork, setRefreshingNetwork] = useState(false);
  const lastNetworkRefresh = useRef(0);
  const liveNetwork = refreshedNetwork ?? network;
  const networkReady = liveNetwork?.state === "ready";

  const refreshNetwork = useCallback(async () => {
    if (!authenticated || refreshingNetwork) return;
    setRefreshingNetwork(true);
    try {
      const response = await fetch("/api/zerant/zcash/network/readiness", {
        credentials: "same-origin",
        cache: "no-store",
      });
      if (!response.ok) return;
      const value = await response.json() as WorkspaceZcashNetwork;
      if (value && ["ready", "syncing", "degraded", "not_configured"].includes(value.state)) {
        setRefreshedNetwork(value);
        onNetworkUpdate?.(value);
        lastNetworkRefresh.current = Date.now();
      }
    } finally {
      setRefreshingNetwork(false);
    }
  }, [authenticated, onNetworkUpdate, refreshingNetwork]);

  useEffect(() => {
    if (!authenticated) return;
    const refreshIfStale = () => {
      if (Date.now() - lastNetworkRefresh.current >= 30_000) void refreshNetwork();
    };
    const onVisibility = () => { if (document.visibilityState === "visible") refreshIfStale(); };
    window.addEventListener("focus", refreshIfStale);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      window.removeEventListener("focus", refreshIfStale);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [authenticated, refreshNetwork]);

  return (
    <section className="zcash-live-status" aria-label="Zcash workflow status">
      <div className="zcash-live-status-head">
        <div>
          <p className="eyebrow">Live workflow</p>
          <h2>From payment details to wallet approval.</h2>
        </div>
        <div className="zcash-live-network-actions">
          <span className={networkReady ? "zcash-live-network is-ready" : "zcash-live-network"}>
            <span aria-hidden="true" />
            {networkReady ? "Testnet ready" : liveNetwork?.state === "syncing" ? "Testnet syncing" : "Network protected"}
          </span>
          {authenticated ? <Button variant="secondary" disabled={refreshingNetwork} onClick={() => void refreshNetwork()}>{refreshingNetwork ? "Checking…" : "Refresh network"}</Button> : null}
        </div>
      </div>

      <div className="zcash-flow-rail">
        <article className={authenticated ? "zcash-flow-step is-complete" : "zcash-flow-step"}>
          <span className="zcash-flow-number">01</span>
          <div><strong>Zerant account</strong><span>{authenticated ? "Connected" : "Sign in required"}</span></div>
        </article>
        <span className="zcash-flow-line" aria-hidden="true" />
        <article className="zcash-flow-step">
          <span className="zcash-flow-number">02</span>
          <div><strong>Prepare</strong><span>Enter exact testnet payment details</span></div>
        </article>
        <span className="zcash-flow-line" aria-hidden="true" />
        <article className="zcash-flow-step is-next">
          <span className="zcash-flow-number">03</span>
          <div><strong>Review and open</strong><span>Approve in your external wallet</span></div>
        </article>
        <span className="zcash-flow-line" aria-hidden="true" />
        <article className="zcash-flow-step is-next">
          <span className="zcash-flow-number">04</span>
          <div><strong>Track</strong><span>Submission and network observation stay separate</span></div>
        </article>
      </div>
    </section>
  );
}
