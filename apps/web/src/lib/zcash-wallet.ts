export type ZcashRequestArguments = {
  method: string;
  params?: unknown[];
};

export interface InjectedZcashProvider {
  request(args: ZcashRequestArguments): Promise<unknown>;
  on?(event: string, handler: (...args: unknown[]) => void): void;
  removeListener?(event: string, handler: (...args: unknown[]) => void): void;
}

type RawNoirWallet = {
  isNoirWallet?: boolean;
  version?: string;
  zcash?: InjectedZcashProvider;
};

export type ZcashAddresses = {
  transparent: string;
  shielded: string;
};

export type ConnectedZcashWallet = {
  providerId: "noir";
  providerName: "Noir Wallet";
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
  return value;
}

declare global {
  interface Window {
    noirwallet?: RawNoirWallet;
  }
}

function stringField(record: Record<string, unknown>, key: string): string {
  const value = record[key];
  if (typeof value !== "string" || !value.trim()) {
    throw new Error("Wallet returned an invalid " + key + ".");
  }
  return value;
}

function recordValue(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("Wallet returned an invalid response.");
  }
  return value as Record<string, unknown>;
}

export function normalizeWalletConnection(value: unknown): ConnectedZcashWallet {
  const record = recordValue(value);
  const shielded = stringField(record, "shielded");
  const transparent = stringField(record, "transparent");
  const accounts = Array.isArray(record.accounts) ? record.accounts : [];
  return {
    providerId: "noir",
    providerName: "Noir Wallet",
    shieldedAddress: shielded,
    transparentAddress: transparent,
    accountCount: Math.max(accounts.length, 1),
  };
}

export function normalizeDerivedSignature(value: unknown): WalletMessageSignature {
  const record = recordValue(value);
  const signingMode = stringField(record, "signingMode");
  if (signingMode !== "derived") {
    throw new Error("Wallet did not use the private identity signing mode.");
  }
  return {
    pubkey: stringField(record, "pubkey"),
    signature: stringField(record, "signature"),
    signingMode: "derived",
  };
}

export class NoirInjectedWallet {
  readonly id = "noir";
  readonly name = "Noir Wallet";

  constructor(private readonly provider: InjectedZcashProvider) {}

  async connect(): Promise<ConnectedZcashWallet> {
    const result = await this.provider.request({ method: "zcash_requestAccounts" });
    return normalizeWalletConnection(result);
  }

  async existingConnection(): Promise<ConnectedZcashWallet | null> {
    const result = await this.provider.request({ method: "zcash_getAccounts" });
    if (result === null) return null;
    return normalizeWalletConnection(result);
  }

  async signIdentityChallenge(message: string): Promise<WalletMessageSignature> {
    const result = await this.provider.request({
      method: "zcash_signMessage",
      params: [message, { signingMode: "derived" }],
    });
    return normalizeDerivedSignature(result);
  }

  async sendShieldedPayment(payment: ShieldedPayment): Promise<string> {
    const result = await this.provider.request({
      method: "zcash_sendTransaction",
      params: [
        {
          to: payment.to,
          amount: payment.amount,
          fundingSource: "shielded",
        },
      ],
    });
    return normalizeTransactionId(result);
  }

  async disconnect(): Promise<void> {
    await this.provider.request({ method: "zcash_disconnect" });
  }
}

export function getInjectedZcashWallet(): NoirInjectedWallet | null {
  if (typeof window === "undefined") return null;
  const raw = window.noirwallet;
  if (!raw?.zcash || raw.isNoirWallet === false) return null;
  return new NoirInjectedWallet(raw.zcash);
}
