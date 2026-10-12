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

export type WalletCapability =
  | "identitySigning" | "zecAuth" | "shieldedPayment"
  | "transparentPayment" | "zip321Handoff" | "connectionRestore";
export type WalletTransport = "injected" | "zecauth" | "uri_handoff";
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

function liveConnector(adapter: ZcashWalletAdapter, transport: "injected"): ZcashConnector {
  const capabilities = new Set<WalletCapability>();
  if (adapter.capabilities.identitySigning && adapter.signIdentityChallenge) capabilities.add("identitySigning");
  if (adapter.capabilities.shieldedPayment && adapter.sendShieldedPayment) capabilities.add("shieldedPayment");
  if (adapter.capabilities.transparentPayment && adapter.sendTransparentPayment) capabilities.add("transparentPayment");
  if (adapter.capabilities.connectionRestore) capabilities.add("connectionRestore");
  return {
    id: `${adapter.id}:${transport}`, walletId: adapter.id, name: adapter.name,
    transport, availability: "detected", capabilities, adapter,
    connect: async () => { const account = await adapter.ensureConnection(); return account; },
    existingConnection: async () => { const account = await adapter.existingConnection(); return account; },
    signIdentityChallenge: adapter.signIdentityChallenge?.bind(adapter),
    sendShieldedPayment: adapter.sendShieldedPayment?.bind(adapter),
    sendTransparentPayment: adapter.sendTransparentPayment?.bind(adapter),
    disconnect: adapter.disconnect ? async () => { await adapter.disconnect!(); } : undefined,
  };
}

export function injectedConnector(adapter: InjectedZcashWalletAdapter): ZcashConnector {
  return liveConnector(adapter, "injected");
}

export function zecAuthConnector(): ZcashConnector {
  return {
    id: "zecauth:portable", walletId: "portable-zecauth", name: "Compatible Zcash sign-in wallet",
    transport: "zecauth", availability: "available",
    capabilities: new Set(["zecAuth"]),
    openAuthHandoff: buildZecAuthWalletUri,
  };
}

export function zip321Connector(): ZcashConnector {
  return {
    id: "zip321:portable", walletId: "portable-payment", name: "Use another Zcash wallet",
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
  return [...capabilities].every((capability) =>
    Object.hasOwn(methods, capability)) &&
    [...capabilities].every((capability) =>
      !methods[capability] || typeof connector[methods[capability]] === "function");
}

export function supportsPurpose(connector: ZcashConnector, purpose: WalletPurpose): boolean {
  if (connector.availability === "unavailable" || connector.availability === "unsupported") return false;
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
  candidates?: ZcashConnector[],
): ZcashConnector[] {
  const available = candidates ?? [
    ...getInjectedZcashWallets().map(injectedConnector),
    ...[...registered.values()].map((factory) => factory()).filter((value): value is ZcashConnector => value !== null),
    zip321Connector(),
  ];
  const priority: Record<WalletAvailability, number> = {
    detected: 0, available: 1, unavailable: 2, unsupported: 3,
  };
  const best = new Map<string, ZcashConnector>();
  for (const connector of available) {
    if (!valid(connector) || !supportsPurpose(connector, purpose)) continue;
    const prior = best.get(connector.walletId);
    if (!prior || priority[connector.availability] < priority[prior.availability]) best.set(connector.walletId, connector);
  }
  const transportPriority: Record<WalletTransport, number> = {
    injected: 0,
    uri_handoff: 1,
    zecauth: 2,
  };
  return [...best.values()].sort((a, b) =>
    priority[a.availability] - priority[b.availability]
    || transportPriority[a.transport] - transportPriority[b.transport]
    || a.name.localeCompare(b.name));
}
