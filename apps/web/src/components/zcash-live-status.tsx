"use client";

import { useZcashConnection } from "@/lib/zcash-connection";

export function ZcashLiveStatus({
  authenticated,
  networkState,
}: {
  authenticated: boolean;
  networkState: "ready" | "syncing" | "degraded" | "not_configured" | null;
}) {
  const connection = useZcashConnection();
  const walletConnected = connection.status === "connected" && Boolean(connection.account);
  const networkReady = networkState === "ready";

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
        <span className={networkReady ? "zcash-live-network is-ready" : "zcash-live-network"}>
          <span aria-hidden="true" />
          {networkReady ? "Testnet ready" : networkState === "syncing" ? "Testnet syncing" : "Network protected"}
        </span>
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
