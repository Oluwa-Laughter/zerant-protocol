import {
  buildZecAuthWalletUri,
  getInjectedZcashWallets,
  type ConnectedZcashWallet,
  type InjectedZcashWalletAdapter,
  type ShieldedPayment,
  type ZcashWalletAdapter,
  type WalletMessageSignature,
  type ZecAuthWalletChallenge,
} from "./zcash-wallet";
import { WalletConnectZcashAdapter } from "./zcash-walletconnect";

export type WalletCapability =
  | "identitySigning" | "zecAuth" | "shieldedPayment"
  | "transparentPayment" | "zip321Handoff" | "walletConnect" | "connectionRestore";
export type WalletTransport = "injected" | "zecauth" | "walletconnect" | "uri_handoff";
export type WalletAvailability = "detected" | "available" | "unavailable" | "unsupported";
export type WalletPurpose = "identity" | "payment" | "connection";
export type ZcashChain = "zcash:mainnet" | "zcash:testnet";

export interface ZcashConnector {
  readonly id: string;
  readonly walletId: string;
  readonly name: string;
  readonly transport: WalletTransport;
  readonly availability: WalletAvailability;
  readonly capabilities: ReadonlySet<WalletCapability>;
  readonly adapter?: ZcashWalletAdapter;
  connect?(): Promise<ConnectedZcashWallet>;
  existingConnection?(): Promise<ConnectedZcashWallet | null>;
  signIdentityChallenge?(message: string): Promise<WalletMessageSignature>;
  sendShieldedPayment?(payment: ShieldedPayment): Promise<string>;
  sendTransparentPayment?(payment: ShieldedPayment): Promise<string>;
  openAuthHandoff?(challenge: ZecAuthWalletChallenge, callback: string): string;
  openPaymentHandoff?(canonicalUri: string): string;
  disconnect?(): Promise<void>;
}

function liveConnector(adapter: ZcashWalletAdapter, transport: "injected" | "walletconnect"): ZcashConnector {
  const capabilities = new Set<WalletCapability>();
  if (transport === "injected" && adapter.capabilities.identitySigning && adapter.signIdentityChallenge) capabilities.add("identitySigning");
  if (transport === "injected" && adapter.capabilities.shieldedPayment && adapter.sendShieldedPayment) capabilities.add("shieldedPayment");
  if (transport === "injected" && adapter.capabilities.transparentPayment && adapter.sendTransparentPayment) capabilities.add("transparentPayment");
  if (adapter.capabilities.connectionRestore) capabilities.add("connectionRestore");
  if (transport === "walletconnect") capabilities.add("walletConnect");
  function syncAfterSession(): void {
    if (transport !== "walletconnect") return;
    capabilities.delete("transparentPayment");
    if (adapter.capabilities.transparentPayment && adapter.sendTransparentPayment) capabilities.add("transparentPayment");
  }
  return {
    id: `${adapter.id}:${transport}`, walletId: adapter.id, name: adapter.name,
    transport, availability: transport === "injected" ? "detected" : "available", capabilities, adapter,
    connect: async () => { const account = await adapter.ensureConnection(); syncAfterSession(); return account; },
    existingConnection: async () => { const account = await adapter.existingConnection(); syncAfterSession(); return account; },
    signIdentityChallenge: adapter.signIdentityChallenge?.bind(adapter),
    sendShieldedPayment: adapter.sendShieldedPayment?.bind(adapter),
    sendTransparentPayment: adapter.sendTransparentPayment?.bind(adapter),
    disconnect: adapter.disconnect ? async () => { try { await adapter.disconnect!(); } finally { capabilities.delete("transparentPayment"); } } : undefined,
  };
}

export function injectedConnector(adapter: InjectedZcashWalletAdapter): ZcashConnector {
  return liveConnector(adapter, "injected");
}

export function walletConnectConnector(projectId: string, displayUri: (uri: string) => void,
  adapter = new WalletConnectZcashAdapter(projectId, displayUri)): ZcashConnector {
  return liveConnector(adapter, "walletconnect");
}

export function zecAuthConnector(): ZcashConnector {
  return {
    id: "zecauth:portable", walletId: "portable-zecauth", name: "ZecAuth-compatible wallet",
    transport: "zecauth", availability: "available",
    capabilities: new Set(["zecAuth"]),
    openAuthHandoff: buildZecAuthWalletUri,
  };
}

export function zip321Connector(): ZcashConnector {
  return {
    id: "zip321:portable", walletId: "portable-payment", name: "Zcash payment wallet",
    transport: "uri_handoff", availability: "available",
    capabilities: new Set(["zip321Handoff"]),
    openPaymentHandoff: (uri) => {
      if (!uri.startsWith("zcash:")) throw new Error("Expected a canonical Zcash payment URI.");
      return uri;
    },
  };
}

const registered = new Map<string, () => ZcashConnector | null>();

export function registerZcashConnector(id: string, factory: () => ZcashConnector | null): () => void {
  if (registered.has(id)) throw new Error(`Wallet connector ${id} is already registered.`);
  registered.set(id, factory);
  return () => { if (registered.get(id) === factory) registered.delete(id); };
}

function valid(connector: ZcashConnector): boolean {
  const methods: Partial<Record<WalletCapability, keyof ZcashConnector>> = {
    identitySigning: "signIdentityChallenge", zecAuth: "openAuthHandoff",
    shieldedPayment: "sendShieldedPayment", zip321Handoff: "openPaymentHandoff",
    transparentPayment: "sendTransparentPayment",
    connectionRestore: "existingConnection",
  };
  if (!connector.id || !connector.walletId || !connector.name || !(connector.capabilities instanceof Set)) return false;
  const capabilities: ReadonlySet<WalletCapability> = connector.capabilities;
  if (capabilities.has("walletConnect") && (connector.transport !== "walletconnect" || !connector.connect)) return false;
  return [...capabilities].every((capability) =>
    Object.hasOwn(methods, capability) || capability === "walletConnect") &&
    [...capabilities].every((capability) =>
      !methods[capability] || typeof connector[methods[capability]] === "function");
}

export function supportsPurpose(connector: ZcashConnector, purpose: WalletPurpose): boolean {
  if (connector.availability === "unavailable" || connector.availability === "unsupported") return false;
  if (purpose === "identity" && connector.transport === "walletconnect") return false;
  if (purpose === "connection") return !!connector.connect || connector.capabilities.has("zecAuth") || connector.capabilities.has("zip321Handoff");
  return purpose === "identity"
    ? (connector.capabilities.has("identitySigning") && !!connector.signIdentityChallenge)
      || (connector.capabilities.has("zecAuth") && !!connector.openAuthHandoff)
    : (connector.capabilities.has("shieldedPayment") && !!connector.sendShieldedPayment)
      || (connector.capabilities.has("transparentPayment") && !!connector.sendTransparentPayment)
      || (connector.capabilities.has("zip321Handoff") && !!connector.openPaymentHandoff);
}

export function discoverZcashConnectors(
  purpose: WalletPurpose,
  activeChain: ZcashChain,
  walletConnectProjectId = "",
  candidates?: ZcashConnector[],
  displayUri: (uri: string) => void = () => {},
): ZcashConnector[] {
  const available = candidates ?? [
    ...getInjectedZcashWallets().map(injectedConnector),
    ...[...registered.values()].map((factory) => factory()).filter((value): value is ZcashConnector => value !== null),
    ...(activeChain === "zcash:mainnet" && walletConnectProjectId.trim() ? [walletConnectConnector(walletConnectProjectId, displayUri)] : []),
    zecAuthConnector(), zip321Connector(),
  ];
  const priority: Record<WalletAvailability, number> = {
    detected: 0, available: 1, unavailable: 2, unsupported: 3,
  };
  const best = new Map<string, ZcashConnector>();
  for (const connector of available) {
    if (connector.transport === "walletconnect" && (activeChain !== "zcash:mainnet" || !walletConnectProjectId.trim())) continue;
    if (!valid(connector) || !supportsPurpose(connector, purpose)) continue;
    const prior = best.get(connector.walletId);
    if (!prior || priority[connector.availability] < priority[prior.availability]) best.set(connector.walletId, connector);
  }
  return [...best.values()].sort((a, b) =>
    priority[a.availability] - priority[b.availability] || a.name.localeCompare(b.name));
}
