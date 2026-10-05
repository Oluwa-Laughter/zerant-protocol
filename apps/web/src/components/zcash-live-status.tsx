"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import type { WorkspaceZcashNetwork } from "@/components/product-workspace";
import { useZcashConnection } from "@/lib/zcash-connection";

export function ZcashLiveStatus({
  authenticated,
  network,
  onNetworkUpdate,
}: {
  authenticated: boolean;
  network: WorkspaceZcashNetwork;
  onNetworkUpdate?: (network: WorkspaceZcashNetwork) => void;
}) {
  const connection = useZcashConnection();
  const [liveNetwork, setLiveNetwork] = useState(network);
  const [refreshingNetwork, setRefreshingNetwork] = useState(false);
  const lastNetworkRefresh = useRef(0);
  const walletConnected = connection.status === "connected" && Boolean(connection.account);
  const networkReady = liveNetwork?.state === "ready";

  const refreshNetwork = useCallback(async (announce = false) => {
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
        setLiveNetwork(value);
        onNetworkUpdate?.(value);
        lastNetworkRefresh.current = Date.now();
      }
    } finally {
      setRefreshingNetwork(false);
    }
  }, [authenticated, onNetworkUpdate, refreshingNetwork]);

  useEffect(() => {
    setLiveNetwork(network);
  }, [network]);

  useEffect(() => {
    if (!authenticated) return;
    const refreshIfStale = () => {
      if (Date.now() - lastNetworkRefresh.current >= 30_000) void refreshNetwork(false);
    };
    const onVisibility = () => { if (document.visibilityState === "visible") refreshIfStale(); };
    window.addEventListener("focus", refreshIfStale);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      window.removeEventListener("focus", refreshIfStale);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [authenticated, refreshNetwork]);

  const walletLabel = walletConnected
    ? connection.account?.providerName ?? "Zcash wallet"
    : connection.status === "connecting"
      ? "Waiting for wallet approval"
      : "Wallet not connected";

  return (
    <section className="zcash-live-status" aria-label="Zcash workflow status">
      <div className="zcash-live-status-head">
        <div>
          <p className="eyebrow">Live workflow</p>
          <h2>From Zerant account to Zcash payment.</h2>
        </div>
        <div className="zcash-live-network-actions">
          <span className={networkReady ? "zcash-live-network is-ready" : "zcash-live-network"}>
            <span aria-hidden="true" />
            {networkReady ? "Testnet ready" : liveNetwork?.state === "syncing" ? "Testnet syncing" : "Network protected"}
          </span>
          {authenticated ? <Button variant="secondary" disabled={refreshingNetwork} onClick={() => void refreshNetwork(true)}>{refreshingNetwork ? "Checking…" : "Refresh network"}</Button> : null}
        </div>
      </div>

      <div className="zcash-flow-rail">
        <article className={authenticated ? "zcash-flow-step is-complete" : "zcash-flow-step"}>
          <span className="zcash-flow-number">01</span>
          <div><strong>Zerant account</strong><span>{authenticated ? "Connected" : "Sign in required"}</span></div>
        </article>
        <span className="zcash-flow-line" aria-hidden="true" />
        <article className={walletConnected ? "zcash-flow-step is-complete" : connection.status === "connecting" ? "zcash-flow-step is-active" : "zcash-flow-step"}>
          <span className="zcash-flow-number">02</span>
          <div><strong>Testnet wallet</strong><span>{walletLabel}</span></div>
        </article>
        <span className="zcash-flow-line" aria-hidden="true" />
        <article className="zcash-flow-step is-next">
          <span className="zcash-flow-number">03</span>
          <div><strong>Private payment</strong><span>Prepare and review exact details</span></div>
        </article>
        <span className="zcash-flow-line" aria-hidden="true" />
        <article className="zcash-flow-step is-next">
          <span className="zcash-flow-number">04</span>
          <div><strong>Track submission</strong><span>Saved by Zerant; settlement remains separate</span></div>
        </article>
      </div>
    </section>
  );
}
