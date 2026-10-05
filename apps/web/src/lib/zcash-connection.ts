"use client";

import { useSyncExternalStore } from "react";
import { connectedWalletNetwork, type ConnectedZcashWallet, type ZcashWalletAdapter } from "./zcash-wallet";
import type { ZcashChain, ZcashConnector } from "./zcash-connectors";

export type ConnectionSnapshot = {
  activeChain: ZcashChain | null;
  walletConnectProjectId: string;
  selected: ZcashConnector | null;
  adapter: ZcashWalletAdapter | null;
  account: ConnectedZcashWallet | null;
  status: "idle" | "connecting" | "connected";
  displayUri: string | null;
};
const empty: ConnectionSnapshot = { activeChain: null, walletConnectProjectId: process.env.NEXT_PUBLIC_ZCASH_WALLETCONNECT_PROJECT_ID ?? "", selected: null, adapter: null, account: null, status: "idle", displayUri: null };
let snapshot = empty;
let configLoading: Promise<ZcashChain> | null = null;
const listeners = new Set<() => void>();
function update(change: Partial<ConnectionSnapshot>) {
  snapshot = { ...snapshot, ...change };
  for (const listener of listeners) listener();
}
export function getZcashConnectionSnapshot(): ConnectionSnapshot { return snapshot; }
export function subscribeZcashConnection(listener: () => void): () => void {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}
export function useZcashConnection(): ConnectionSnapshot {
  return useSyncExternalStore(subscribeZcashConnection, getZcashConnectionSnapshot, () => empty);
}

export async function ensureZcashConfig(request: typeof fetch = fetch): Promise<ZcashChain> {
  if (snapshot.activeChain) return snapshot.activeChain;
  configLoading ??= (async () => {
    const response = await request("/api/zerant/public/zcash/config", { credentials: "same-origin", cache: "no-store" });
    if (!response.ok) throw new Error("Zcash network configuration is unavailable.");
    const value: unknown = await response.json();
    const chain = (value as { chain?: unknown } | null)?.chain;
    if (chain !== "zcash:mainnet" && chain !== "zcash:testnet") throw new Error("Invalid Zcash network configuration.");
    update({ activeChain: chain });
    return chain;
  })();
  try { return await configLoading; } finally { configLoading = null; }
}

export function setZcashDisplayUri(uri: string): void { update({ displayUri: uri }); }

function requireWalletNetwork(account: ConnectedZcashWallet, chain: ZcashChain): void {
  if (connectedWalletNetwork(account) !== chain) {
    const expected = chain === "zcash:testnet" ? "testnet" : "mainnet";
    throw new Error(`This wallet account does not match Zerant’s ${expected} network. Choose a wallet on ${expected}, or open the reviewed payment request in another wallet.`);
  }
}

export async function connectConnector(connector: ZcashConnector, chain: ZcashChain): Promise<ConnectedZcashWallet> {
  if (!connector.adapter || !connector.connect) throw new Error("This choice does not support a live wallet connection.");
  update({ selected: connector, adapter: connector.adapter, account: null, status: "connecting", displayUri: null });
  try {
    const account = await connector.connect();
    try {
      requireWalletNetwork(account, chain);
    } catch (error) {
      await connector.disconnect?.().catch(() => undefined);
      throw error;
    }
    update({ account, status: "connected" });
    return account;
  } catch (error) {
    update({ selected: null, adapter: null, account: null, status: "idle" });
    throw error;
  }
}

export async function restoreConnection(connectors: ZcashConnector[], chain: ZcashChain): Promise<ConnectedZcashWallet | null> {
  for (const connector of connectors) {
    if (!connector.adapter || !connector.capabilities.has("connectionRestore") || !connector.existingConnection) continue;
    try {
      const account = await connector.existingConnection();
      if (account) { requireWalletNetwork(account, chain); update({ selected: connector, adapter: connector.adapter, account, status: "connected" }); return account; }
    } catch { /* A stale session must not prevent another adapter restoring. */ }
  }
  return null;
}

export async function disconnectZcash(): Promise<void> {
  try { await snapshot.selected?.disconnect?.(); }
  finally { update({ selected: null, adapter: null, account: null, status: "idle", displayUri: null }); }
}
