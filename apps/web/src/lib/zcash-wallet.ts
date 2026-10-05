import {
  getNoirWallet,
  type NoirWalletProvider,
  type SignMessageResult,
  type ZcashConnectResult,
} from "@noir-wallet/sdk";

export type ConnectedZcashWallet = {
  providerId: string;
  providerName: string;
  shieldedAddress: string;
  transparentAddress: string;
  accountCount: number;
};

export type WalletMessageSignature = {
  pubkey: string;
  signature: string;
  signingMode: "derived";
};

export type ShieldedPayment = {
  to: string;
  amount: string;
};

export type ZcashWalletCapabilities = {
  identitySigning: boolean;
  shieldedPayment: boolean;
  transparentPayment: boolean;
  paymentRequestHandoff: boolean;
  walletConnect: boolean;
  zecAuthHandoff: boolean;
  connectionRestore: boolean;
};

export interface ZcashWalletAdapter {
  readonly id: string;
  readonly name: string;
  readonly capabilities: ZcashWalletCapabilities;
  connect(): Promise<ConnectedZcashWallet>;
  existingConnection(): Promise<ConnectedZcashWallet | null>;
  ensureConnection(): Promise<ConnectedZcashWallet>;
  signIdentityChallenge?(message: string): Promise<WalletMessageSignature>;
  sendShieldedPayment?(payment: ShieldedPayment): Promise<string>;
  sendTransparentPayment?(payment: ShieldedPayment): Promise<string>;
  disconnect?(): Promise<void>;
}

/** Kept for existing account-link integrations. */
export type InjectedZcashWalletAdapter = ZcashWalletAdapter;

export type ZecAuthWalletChallenge = {
  domain: string;
  uri: string;
  version: number;
  chain: string;
  nonce: string;
  issued_at: string;
  expiration_time: string;
  statement: string;
  scopes: { required: Array<{ type: string }> };
};

export function zatoshiToZec(zat: number): string {
  if (!Number.isSafeInteger(zat) || zat <= 0) {
    throw new Error("Payment amount must be a positive safe integer.");
  }
  const whole = Math.floor(zat / 100_000_000);
  const fraction = (zat % 100_000_000).toString().padStart(8, "0").replace(/0+$/, "");
  return fraction ? whole.toString() + "." + fraction : whole.toString();
}

function normalizeTransactionId(value: unknown): string {
  if (typeof value !== "string" || !value.trim()) {
    throw new Error("Wallet did not return a transaction id.");
  }
  return value.trim();
}

export function normalizeWalletConnection(
  value: ZcashConnectResult | unknown,
  providerId = "injected",
  providerName = "Zcash wallet",
): ConnectedZcashWallet {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("Wallet returned an invalid account response.");
  }

  const record = value as Record<string, unknown>;
  const shielded = typeof record.shielded === "string" ? record.shielded.trim() : "";
  const transparent = typeof record.transparent === "string" ? record.transparent.trim() : "";
  if (!shielded && !transparent) {
    throw new Error("Wallet did not return a usable Zcash account.");
  }

  const accounts = Array.isArray(record.accounts) ? record.accounts : [];
  return {
    providerId,
    providerName,
    shieldedAddress: shielded,
    transparentAddress: transparent,
    accountCount: Math.max(accounts.length, 1),
  };
}

export function normalizeDerivedSignature(
  value: SignMessageResult | unknown,
): WalletMessageSignature {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("Wallet returned an invalid signature response.");
  }

  const record = value as Record<string, unknown>;
  const signingMode = record.signingMode;
  const pubkey = typeof record.pubkey === "string" ? record.pubkey.trim() : "";
  const signature = typeof record.signature === "string" ? record.signature.trim() : "";

  if (signingMode !== "derived") {
    throw new Error("Wallet did not use Zerant's private identity signing mode.");
  }
  if (!pubkey || !signature) {
    throw new Error("Wallet returned an incomplete identity signature.");
  }

  return { pubkey, signature, signingMode: "derived" };
}

export function buildZecAuthWalletUri(
  challenge: ZecAuthWalletChallenge,
  callbackUrl: string,
): string {
  if (!challenge.domain.trim() || challenge.version !== 1) {
    throw new Error("Invalid Zcash authentication challenge.");
  }

  const callback = new URL(callbackUrl);
  if (!["https:", "http:"].includes(callback.protocol)) {
    throw new Error("Invalid authentication callback.");
  }

  const payload = {
    domain: challenge.domain,
    uri: challenge.uri,
    version: challenge.version,
    chain: challenge.chain,
    nonce: challenge.nonce,
    issued_at: challenge.issued_at,
    expiration_time: challenge.expiration_time,
    statement: challenge.statement,
    scopes: challenge.scopes,
  };

  return (
    "zecauth://" +
    challenge.domain +
    "?challenge=" +
    encodeURIComponent(JSON.stringify(payload)) +
    "&callback=" +
    encodeURIComponent(callback.toString())
  );
}

export class NoirWalletAdapter implements ZcashWalletAdapter {
  readonly id = "noir";
  readonly name = "Noir Wallet";
  readonly capabilities: ZcashWalletCapabilities = {
    identitySigning: true,
    shieldedPayment: true,
    transparentPayment: false,
    paymentRequestHandoff: false,
    walletConnect: false,
    zecAuthHandoff: false,
    connectionRestore: true,
  };

  constructor(private readonly wallet: NoirWalletProvider) {}

  async connect(): Promise<ConnectedZcashWallet> {
    const result = await this.wallet.zcash.connect();
    return normalizeWalletConnection(result, this.id, this.name);
  }

  async existingConnection(): Promise<ConnectedZcashWallet | null> {
    const result = await this.wallet.zcash.getAccounts();
    return result ? normalizeWalletConnection(result, this.id, this.name) : null;
  }

  async ensureConnection(): Promise<ConnectedZcashWallet> {
    // A user click is the authorization gesture. Some extension versions reject
    // zcash_getAccounts before approval instead of returning null, which must not
    // prevent the zcash_requestAccounts approval prompt from opening.
    return this.connect();
  }

  async signIdentityChallenge(message: string): Promise<WalletMessageSignature> {
    const result = await this.wallet.zcash.signMessage(message, {
      signingMode: "derived",
    });
    return normalizeDerivedSignature(result);
  }

  async sendShieldedPayment(payment: ShieldedPayment): Promise<string> {
    const result = await this.wallet.zcash.sendTransaction({
      to: payment.to,
      amount: payment.amount,
      fundingSource: "shielded",
    });
    return normalizeTransactionId(result);
  }

  async disconnect(): Promise<void> {
    await this.wallet.zcash.disconnect();
  }
}

export function detectNoirWallet(): ZcashWalletAdapter | null {
  const wallet = getNoirWallet();
  return wallet ? new NoirWalletAdapter(wallet) : null;
}

export type ZcashWalletDetector = () => ZcashWalletAdapter | null;
const browserDetectors = new Set<ZcashWalletDetector>([detectNoirWallet]);

/** Register explicit, reviewed provider detectors. Never enumerate arbitrary window keys. */
export function registerZcashWalletDetector(detector: ZcashWalletDetector): () => void {
  browserDetectors.add(detector);
  return () => { browserDetectors.delete(detector); };
}

export function getInjectedZcashWallets(): ZcashWalletAdapter[] {
  if (typeof window === "undefined") return [];
  const adapters = [...browserDetectors].map((detect) => detect()).filter(
    (adapter): adapter is ZcashWalletAdapter => adapter !== null,
  );
  return [...new Map(adapters.map((adapter) => [adapter.id, adapter])).values()];
}
