export const VAULT_VERSION = 1 as const;
export const KDF_ITERATIONS = 600_000;
const MAX_PASSPHRASE_BYTES = 1024;
const MIN_PASSPHRASE_CHARS = 12;
const MAX_RECORD_BYTES = 262_144;
const MAX_RECORDS = 256;

export type VaultCipherRecordV1 = {
  version: 1;
  iv: string;
  ciphertext: string;
};

export type VaultEnvelopeV1 = {
  version: 1;
  kdf: {
    name: "PBKDF2";
    hash: "SHA-256";
    iterations: number;
    salt: string;
  };
  wrappedKey: {
    algorithm: "AES-GCM";
    iv: string;
    ciphertext: string;
  };
  records: Record<string, VaultCipherRecordV1>;
};

export class VaultError extends Error {
  constructor(message = "Vault operation failed") {
    super(message);
    this.name = "VaultError";
  }
}

const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });

function cryptoOrThrow(provider?: Crypto): Crypto {
  const value = provider ?? globalThis.crypto;
  if (!value?.subtle || !value.getRandomValues) throw new VaultError("Web Crypto is unavailable");
  return value;
}

function encodeBase64Url(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/u, "");
}

function decodeBase64Url(value: string): Uint8Array {
  if (!value || !/^[A-Za-z0-9_-]+$/u.test(value)) throw new VaultError();
  const padding = "=".repeat((4 - (value.length % 4)) % 4);
  let binary: string;
  try {
    binary = atob(value.replace(/-/g, "+").replace(/_/g, "/") + padding);
  } catch {
    throw new VaultError();
  }
  const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
  if (encodeBase64Url(bytes) !== value) throw new VaultError();
  return bytes;
}

function randomBytes(length: number, cryptoProvider: Crypto): Uint8Array {
  return cryptoProvider.getRandomValues(new Uint8Array(length));
}

function ownedBuffer(bytes: Uint8Array): ArrayBuffer {
  const copy = new Uint8Array(bytes.byteLength);
  copy.set(bytes);
  return copy.buffer;
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function exactKeys(value: Record<string, unknown>, expected: readonly string[]): boolean {
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  return actual.length === wanted.length && actual.every((key, index) => key === wanted[index]);
}

function validatePassphrase(passphrase: string): Uint8Array {
  if (passphrase.length < MIN_PASSPHRASE_CHARS) {
    throw new VaultError("Use at least 12 characters for the local vault passphrase");
  }
  const bytes = encoder.encode(passphrase);
  if (bytes.byteLength > MAX_PASSPHRASE_BYTES) throw new VaultError("Passphrase is too long");
  return bytes;
}

function validateRecordId(recordId: string): void {
  if (!recordId || recordId.length > 128 || !/^[A-Za-z0-9._:-]+$/u.test(recordId)) {
    throw new VaultError("Invalid record identifier");
  }
}

function recordAad(recordId: string): Uint8Array {
  return encoder.encode(`zerant:vault-record:v1:${recordId}`);
}

const WRAP_AAD = encoder.encode("zerant:vault-key:v1");

async function deriveWrappingKey(
  passphrase: string,
  salt: Uint8Array,
  iterations: number,
  cryptoProvider: Crypto,
): Promise<CryptoKey> {
  const passphraseBytes = validatePassphrase(passphrase);
  try {
    const source = await cryptoProvider.subtle.importKey(
      "raw",
      ownedBuffer(passphraseBytes),
      "PBKDF2",
      false,
      ["deriveKey"],
    );
    return await cryptoProvider.subtle.deriveKey(
      { name: "PBKDF2", salt: ownedBuffer(salt), iterations, hash: "SHA-256" },
      source,
      { name: "AES-GCM", length: 256 },
      false,
      ["encrypt", "decrypt"],
    );
  } finally {
    passphraseBytes.fill(0);
  }
}

function assertEnvelope(value: VaultEnvelopeV1): void {
  const root = value as unknown;
  if (!isPlainObject(root) || !exactKeys(root, ["version", "kdf", "wrappedKey", "records"])) {
    throw new VaultError("Invalid vault envelope");
  }
  if (!isPlainObject(value.kdf) || !exactKeys(value.kdf as unknown as Record<string, unknown>, ["name", "hash", "iterations", "salt"])) {
    throw new VaultError("Invalid vault envelope");
  }
  if (!isPlainObject(value.wrappedKey) || !exactKeys(value.wrappedKey as unknown as Record<string, unknown>, ["algorithm", "iv", "ciphertext"])) {
    throw new VaultError("Invalid vault envelope");
  }
  if (
    value.version !== VAULT_VERSION ||
    value.kdf.name !== "PBKDF2" ||
    value.kdf.hash !== "SHA-256" ||
    !Number.isSafeInteger(value.kdf.iterations) ||
    value.kdf.iterations < KDF_ITERATIONS ||
    value.kdf.iterations > 2_000_000 ||
    value.wrappedKey.algorithm !== "AES-GCM" ||
    !isPlainObject(value.records) ||
    Object.keys(value.records).length > MAX_RECORDS
  ) {
    throw new VaultError("Invalid vault envelope");
  }
  if (value.kdf.salt.length > 24 || decodeBase64Url(value.kdf.salt).byteLength !== 16) {
    throw new VaultError("Invalid vault envelope");
  }
  if (value.wrappedKey.iv.length > 18 || decodeBase64Url(value.wrappedKey.iv).byteLength !== 12) {
    throw new VaultError("Invalid vault envelope");
  }
  if (
    value.wrappedKey.ciphertext.length > 72 ||
    decodeBase64Url(value.wrappedKey.ciphertext).byteLength !== 48
  ) {
    throw new VaultError("Invalid vault envelope");
  }
  const maxEncodedRecord = Math.ceil(((MAX_RECORD_BYTES + 16) * 4) / 3) + 4;
  for (const [recordId, record] of Object.entries(value.records)) {
    validateRecordId(recordId);
    if (
      !isPlainObject(record) ||
      !exactKeys(record as unknown as Record<string, unknown>, ["version", "iv", "ciphertext"]) ||
      record.version !== 1 ||
      record.iv.length > 18 ||
      decodeBase64Url(record.iv).byteLength !== 12 ||
      record.ciphertext.length > maxEncodedRecord ||
      decodeBase64Url(record.ciphertext).byteLength < 16
    ) {
      throw new VaultError("Invalid vault envelope");
    }
  }
}

export function parseVaultEnvelope(value: unknown): VaultEnvelopeV1 {
  if (!value || typeof value !== "object") throw new VaultError("Invalid vault envelope");
  const candidate = value as VaultEnvelopeV1;
  assertEnvelope(candidate);
  return structuredClone(candidate);
}

export function createRecordId(provider?: Crypto): string {
  return encodeBase64Url(randomBytes(16, cryptoOrThrow(provider)));
}

export class VaultSession {
  #key: CryptoKey | null;
  #crypto: Crypto;

  constructor(key: CryptoKey, provider?: Crypto) {
    this.#key = key;
    this.#crypto = cryptoOrThrow(provider);
  }

  get locked(): boolean {
    return this.#key === null;
  }

  lock(): void {
    this.#key = null;
  }

  async sealRecord(
    envelope: VaultEnvelopeV1,
    recordId: string,
    value: unknown,
  ): Promise<VaultEnvelopeV1> {
    const key = this.#key;
    if (!key) throw new VaultError("Vault is locked");
    assertEnvelope(envelope);
    validateRecordId(recordId);

    const records = Object.keys(envelope.records);
    if (!Object.hasOwn(envelope.records, recordId) && records.length >= MAX_RECORDS) {
      throw new VaultError("Vault record limit reached");
    }

    let plaintext: Uint8Array;
    try {
      const json = JSON.stringify(value);
      if (json === undefined) throw new VaultError("Record is not JSON serializable");
      plaintext = encoder.encode(json);
    } catch (error) {
      if (error instanceof VaultError) throw error;
      throw new VaultError("Record is not JSON serializable");
    }
    if (plaintext.byteLength > MAX_RECORD_BYTES) {
      plaintext.fill(0);
      throw new VaultError("Vault record is too large");
    }

    const iv = randomBytes(12, this.#crypto);
    try {
      const encrypted = new Uint8Array(
        await this.#crypto.subtle.encrypt(
          { name: "AES-GCM", iv: ownedBuffer(iv), additionalData: ownedBuffer(recordAad(recordId)), tagLength: 128 },
          key,
          ownedBuffer(plaintext),
        ),
      );
      return {
        ...structuredClone(envelope),
        records: {
          ...structuredClone(envelope.records),
          [recordId]: {
            version: 1,
            iv: encodeBase64Url(iv),
            ciphertext: encodeBase64Url(encrypted),
          },
        },
      };
    } finally {
      plaintext.fill(0);
    }
  }

  async openRecord<T = unknown>(envelope: VaultEnvelopeV1, recordId: string): Promise<T> {
    const key = this.#key;
    if (!key) throw new VaultError("Vault is locked");
    assertEnvelope(envelope);
    validateRecordId(recordId);
    const record = envelope.records[recordId];
    if (!record) throw new VaultError("Vault record not found");

    try {
      const plaintext = new Uint8Array(
        await this.#crypto.subtle.decrypt(
          {
            name: "AES-GCM",
            iv: ownedBuffer(decodeBase64Url(record.iv)),
            additionalData: ownedBuffer(recordAad(recordId)),
            tagLength: 128,
          },
          key,
          ownedBuffer(decodeBase64Url(record.ciphertext)),
        ),
      );
      try {
        return JSON.parse(decoder.decode(plaintext)) as T;
      } finally {
        plaintext.fill(0);
      }
    } catch {
      throw new VaultError("Vault record could not be decrypted");
    }
  }
}

export async function createVault(
  passphrase: string,
  provider?: Crypto,
): Promise<{ envelope: VaultEnvelopeV1; session: VaultSession }> {
  const cryptoProvider = cryptoOrThrow(provider);
  const salt = randomBytes(16, cryptoProvider);
  const dataKeyBytes = randomBytes(32, cryptoProvider);
  const wrappingKey = await deriveWrappingKey(passphrase, salt, KDF_ITERATIONS, cryptoProvider);
  const wrapIv = randomBytes(12, cryptoProvider);

  try {
    const wrapped = new Uint8Array(
      await cryptoProvider.subtle.encrypt(
        { name: "AES-GCM", iv: ownedBuffer(wrapIv), additionalData: ownedBuffer(WRAP_AAD), tagLength: 128 },
        wrappingKey,
        ownedBuffer(dataKeyBytes),
      ),
    );
    const dataKey = await cryptoProvider.subtle.importKey(
      "raw",
      ownedBuffer(dataKeyBytes),
      { name: "AES-GCM", length: 256 },
      false,
      ["encrypt", "decrypt"],
    );
    return {
      envelope: {
        version: VAULT_VERSION,
        kdf: {
          name: "PBKDF2",
          hash: "SHA-256",
          iterations: KDF_ITERATIONS,
          salt: encodeBase64Url(salt),
        },
        wrappedKey: {
          algorithm: "AES-GCM",
          iv: encodeBase64Url(wrapIv),
          ciphertext: encodeBase64Url(wrapped),
        },
        records: {},
      },
      session: new VaultSession(dataKey, cryptoProvider),
    };
  } finally {
    dataKeyBytes.fill(0);
  }
}

export async function unlockVault(
  envelopeValue: VaultEnvelopeV1,
  passphrase: string,
  provider?: Crypto,
): Promise<VaultSession> {
  const cryptoProvider = cryptoOrThrow(provider);
  const envelope = parseVaultEnvelope(envelopeValue);
  const wrappingKey = await deriveWrappingKey(
    passphrase,
    decodeBase64Url(envelope.kdf.salt),
    envelope.kdf.iterations,
    cryptoProvider,
  );

  let raw: Uint8Array | null = null;
  try {
    raw = new Uint8Array(
      await cryptoProvider.subtle.decrypt(
        {
          name: "AES-GCM",
          iv: ownedBuffer(decodeBase64Url(envelope.wrappedKey.iv)),
          additionalData: ownedBuffer(WRAP_AAD),
          tagLength: 128,
        },
        wrappingKey,
        ownedBuffer(decodeBase64Url(envelope.wrappedKey.ciphertext)),
      ),
    );
    if (raw.byteLength !== 32) throw new VaultError();
    const key = await cryptoProvider.subtle.importKey(
      "raw",
      ownedBuffer(raw),
      { name: "AES-GCM", length: 256 },
      false,
      ["encrypt", "decrypt"],
    );
    return new VaultSession(key, cryptoProvider);
  } catch {
    throw new VaultError("Vault could not be unlocked");
  } finally {
    raw?.fill(0);
  }
}

export function recordIds(envelope: VaultEnvelopeV1): string[] {
  assertEnvelope(envelope);
  return Object.keys(envelope.records).sort();
}

export function removeRecord(envelope: VaultEnvelopeV1, recordId: string): VaultEnvelopeV1 {
  assertEnvelope(envelope);
  validateRecordId(recordId);
  const next = structuredClone(envelope);
  delete next.records[recordId];
  return next;
}
