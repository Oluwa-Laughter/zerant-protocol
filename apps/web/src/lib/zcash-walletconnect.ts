import type { ZcashWalletAdapter, ZcashWalletCapabilities, ConnectedZcashWallet, ShieldedPayment } from "./zcash-wallet";

export const ZCASH_MAINNET_CAIP = "bip122:00040fe8ec8471911baa1db1266ea15d";

type Session = {
  topic: string;
  namespaces: Record<string, { accounts: string[]; methods: string[]; chains?: string[] } | undefined>;
};
type SignClientLike = {
  session: { getAll(): Session[] };
  connect(input: unknown): Promise<{ uri?: string; approval(): Promise<Session> }>;
  disconnect(input: unknown): Promise<void>;
  request(input: unknown): Promise<unknown>;
};

export function compatibleZcashSession(session: Session): boolean {
  const namespace = session.namespaces.bip122;
  return Boolean(namespace?.accounts.some((account) => account.startsWith(`${ZCASH_MAINNET_CAIP}:`)) &&
    namespace.methods.includes("zcash_getAddress"));
}

export class WalletConnectZcashAdapter implements ZcashWalletAdapter {
  readonly id = "walletconnect";
  readonly name = "WalletConnect-compatible wallet";
  readonly capabilities: ZcashWalletCapabilities = {
    identitySigning: false, shieldedPayment: false, transparentPayment: false,
    paymentRequestHandoff: false, walletConnect: true, zecAuthHandoff: false,
    connectionRestore: true,
  };
  private client: SignClientLike | null = null;
  private session: Session | null = null;
  private loading: Promise<SignClientLike> | null = null;

  constructor(private readonly projectId: string, private readonly displayUri: (uri: string) => void,
    private readonly makeClient?: () => Promise<SignClientLike>) {
    if (!projectId.trim()) throw new Error("WalletConnect project ID is required.");
  }

  private async getClient(): Promise<SignClientLike> {
    if (this.client) return this.client;
    this.loading ??= (this.makeClient ?? (async () => {
      const { default: SignClient } = await import("@walletconnect/sign-client");
      return SignClient.init({ projectId: this.projectId, metadata: {
        name: "Zerant", description: "Zcash wallet connection", url: window.location.origin, icons: [],
      } }) as Promise<SignClientLike>;
    }))();
    try { this.client = await this.loading; return this.client; }
    catch (error) { this.loading = null; throw error; }
  }

  private useSession(session: Session): ConnectedZcashWallet {
    if (!compatibleZcashSession(session)) throw new Error("WalletConnect session does not expose a compatible Zcash account.");
    this.session = session;
    const namespace = session.namespaces.bip122!;
    const address = namespace.accounts.find((account) => account.startsWith(`${ZCASH_MAINNET_CAIP}:`))!.slice(ZCASH_MAINNET_CAIP.length + 1);
    this.capabilities.transparentPayment = namespace.methods.includes("zcash_transfer") && /^(t1|t3)/.test(address);
    return {
      providerId: this.id, providerName: this.name,
      shieldedAddress: /^(u|zs)/.test(address) ? address : "",
      transparentAddress: /^(t1|t3)/.test(address) ? address : "",
      accountCount: 1,
    };
  }

  async existingConnection(): Promise<ConnectedZcashWallet | null> {
    const client = await this.getClient();
    const session = client.session.getAll().find(compatibleZcashSession);
    if (!session) { this.session = null; this.capabilities.transparentPayment = false; }
    return session ? this.useSession(session) : null;
  }

  async connect(): Promise<ConnectedZcashWallet> {
    const existing = await this.existingConnection();
    if (existing) return existing;
    const client = await this.getClient();
    const { uri, approval } = await client.connect({ requiredNamespaces: { bip122: {
      chains: [ZCASH_MAINNET_CAIP], methods: ["zcash_getAddress"], events: [],
    } }, optionalNamespaces: { bip122: {
      chains: [ZCASH_MAINNET_CAIP], methods: ["zcash_transfer"], events: [],
    } } });
    if (uri) this.displayUri(uri);
    return this.useSession(await approval());
  }

  async ensureConnection(): Promise<ConnectedZcashWallet> { return (await this.existingConnection()) ?? this.connect(); }

  async sendTransparentPayment(payment: ShieldedPayment): Promise<string> {
    if (!this.session || !this.capabilities.transparentPayment) throw new Error("This wallet cannot send a transparent payment through WalletConnect.");
    const result = await (await this.getClient()).request({ topic: this.session.topic, chainId: ZCASH_MAINNET_CAIP,
      request: { method: "zcash_transfer", params: { ...payment, fundingSource: "transparent", type: "transparent" } } });
    const txid = typeof result === "string" ? result : (result as { txid?: unknown } | null)?.txid;
    if (typeof txid !== "string" || !txid) throw new Error("WalletConnect did not return a transaction id.");
    return txid;
  }

  async disconnect(): Promise<void> {
    const session = this.session;
    this.session = null;
    this.capabilities.transparentPayment = false;
    if (session) await (await this.getClient()).disconnect({ topic: session.topic, reason: { code: 6000, message: "User disconnected" } });
  }
}
