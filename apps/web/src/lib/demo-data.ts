// Public UI fixtures, intentionally not wire-format credentials or authenticated requests.
export type PreviewRequest = {
  origin: string; purpose: string; issuer: string; claim: string; context: string;
  policy: string; threshold: number; result: boolean; requestWindow: string; attestationWindow: string;
};
export const previewRequest: PreviewRequest = {
  origin: "https://grants.example", purpose: "Review eligibility for an OSS community grant",
  issuer: "zerant:issuer:local-demo", claim: "reputation.threshold", context: "oss-community",
  policy: "zerant:oss:contribution · v0.1", threshold: 40, result: true,
  requestWindow: "5 minutes from issuance (illustrative; no live timer)",
  attestationWindow: "At most 24 hours, bounded by source validity (illustrative)",
};
export const withheld = ["Exact reputation score", "Source credentials and contribution history", "Other credentials and contexts", "Private source subject key", "Wallet addresses, balances and transactions"] as const;
export const metadata = [
  ["Audience subject public key", "Independent key for this origin · placeholder"],
  ["Credential / revocation IDs", "Fresh attestation identifiers · placeholders"],
  ["Issuer signing key / schema", "Pinned issuer key · zerant.credential.v0.1"],
  ["Policy digest / evaluation time", "Pinned policy digest and as_of · placeholders"],
  ["Validity times / audience", "issued_at, expires_at and grants.example origin"],
  ["Request bindings", "Request ID, digest, challenge, nonce and origin · placeholders"],
  ["Signed envelopes / response time", "Issuer attestation JWS, holder response JWS and responded_at · not generated"],
] as const;
