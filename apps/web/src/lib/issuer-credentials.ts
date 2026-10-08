export type CredentialState = "active" | "expired" | "revoked";
export type CredentialFilter = "all" | CredentialState;

export type IssuedCredentialSummary = {
  credential_schema_id: string | null;
  holder_zerant_id: string;
  claim_type: string;
  context: string;
  issued_at: string;
  expires_at: string;
  revoked: boolean;
};

export type CredentialSchemaSummary = {
  id: string;
  display_name: string;
};

export function credentialState(credential: Pick<IssuedCredentialSummary, "revoked" | "expires_at">, now = Date.now()): CredentialState {
  if (credential.revoked) return "revoked";
  const expiry = Date.parse(credential.expires_at);
  return Number.isFinite(expiry) && expiry > now ? "active" : "expired";
}

export function filterIssuedCredentials<T extends IssuedCredentialSummary>(
  credentials: T[],
  schemas: CredentialSchemaSummary[],
  query: string,
  filter: CredentialFilter = "all",
  now = Date.now(),
): T[] {
  const schemaNames = new Map(schemas.map((schema) => [schema.id, schema.display_name]));
  const normalized = query.trim().toLocaleLowerCase();
  return credentials.filter((credential) => {
    if (filter !== "all" && credentialState(credential, now) !== filter) return false;
    if (!normalized) return true;
    const schemaName = credential.credential_schema_id
      ? schemaNames.get(credential.credential_schema_id) ?? ""
      : "";
    return [
      credential.holder_zerant_id,
      credential.claim_type,
      credential.context,
      schemaName,
    ].some((part) => part.toLocaleLowerCase().includes(normalized));
  }).sort((left, right) => {
    const a = Date.parse(left.issued_at);
    const b = Date.parse(right.issued_at);
    return (Number.isFinite(b) ? b : 0) - (Number.isFinite(a) ? a : 0);
  });
}
