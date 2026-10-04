import sharedScenarios from "../../../../fixtures/scenarios.json";
// Public UI fixtures only. They are not signed wire credentials or authenticated requests.
export type PreviewRequest = {
  origin: string;
  purpose: string;
  issuer: string;
  claim: string;
  context: string;
  policy?: string;
  threshold?: number;
  result: boolean | string;
  requestWindow: string;
  attestationWindow: string;
};

export type Scenario = {
  id: string;
  label: string;
  audience: string;
  request: PreviewRequest;
  sharedSummary: string;
  withheld: string[];
  issuerSummary: string;
  verifierSummary: string;
  zcashStatus?: string;
};

const commonPrivate = [
  "Other credentials and unrelated contexts",
  "Private source subject key",
  "Wallet addresses, balances and transaction history",
];

const scenarioDetails: Scenario[] = [
  {
    id: "freelancer",
    label: "Freelancer",
    audience: "Client / platform",
    request: {
      origin: "https://client.example",
      purpose: "Confirm one completed professional engagement",
      issuer: "zerant:issuer:freelance-demo",
      claim: "service.completed",
      context: "freelance-service",
      result: "completed",
      requestWindow: "5 minutes from issuance",
      attestationWindow: "Bounded by issuer validity",
    },
    sharedSummary: "One issuer-attested service-completion result plus required verification metadata.",
    withheld: ["Other clients and engagements", "Rates and invoice details", ...commonPrivate],
    issuerSummary: "Attest only to the engagement you can substantiate; do not expose the freelancer’s client portfolio.",
    verifierSummary: "Verify one completed service result, not the freelancer’s whole work history.",
  },
  {
    id: "business",
    label: "Business / vendor",
    audience: "Buyer / procurement",
    request: {
      origin: "https://procurement.example",
      purpose: "Check vendor qualification under one reviewed policy",
      issuer: "zerant:issuer:vendor-demo",
      claim: "reputation.threshold",
      context: "vendor-qualification",
      policy: "zerant:business-vendor:eligibility · v0.1",
      threshold: 60,
      result: true,
      requestWindow: "5 minutes from issuance",
      attestationWindow: "At most 24 hours for threshold attestations",
    },
    sharedSummary: "One issuer-attested vendor threshold result: true, plus required verification metadata.",
    withheld: ["Exact contextual score", "Underlying vendor evidence", "Other customers or contracts", ...commonPrivate],
    issuerSummary: "Evaluate only the configured vendor context and sign the narrow supported threshold.",
    verifierSummary: "Accept only the exact policy, digest, threshold, issuer, audience and replay-safe response.",
  },
  {
    id: "community",
    label: "Community",
    audience: "Community application",
    request: {
      origin: "https://community.example",
      purpose: "Confirm active community membership",
      issuer: "zerant:issuer:community-demo",
      claim: "membership.active",
      context: "community-contribution",
      result: true,
      requestWindow: "5 minutes from issuance",
      attestationWindow: "Bounded by issuer validity",
    },
    sharedSummary: "One active-membership attestation for this community.",
    withheld: ["Other communities", "Contribution event history", ...commonPrivate],
    issuerSummary: "Bind membership to this context without creating a global identity profile.",
    verifierSummary: "Request the one role/member fact the application actually needs.",
  },
  {
    id: "grant",
    label: "Grant",
    audience: "Grant program",
    request: {
      origin: "https://grant.example",
      purpose: "Check eligibility for this grant",
      issuer: "zerant:issuer:grant-demo",
      claim: "reputation.threshold",
      context: "grant-eligibility",
      policy: "zerant:grant-eligibility:eligibility · v0.1",
      threshold: 40,
      result: true,
      requestWindow: "5 minutes from issuance",
      attestationWindow: "At most 24 hours for threshold attestations",
    },
    sharedSummary: "One policy-bound eligibility threshold result: true.",
    withheld: ["Exact contextual score", "Source evidence and event IDs", "Unrelated grant applications", ...commonPrivate],
    issuerSummary: "Sign the supported eligibility result only after evaluating your authorized evidence.",
    verifierSummary: "Do not ask for the holder’s source credentials when one eligibility result is sufficient.",
  },
  {
    id: "oss",
    label: "Open source",
    audience: "OSS program",
    request: {
      origin: "https://oss.example",
      purpose: "Check contribution eligibility",
      issuer: "zerant:issuer:oss-demo",
      claim: "reputation.threshold",
      context: "oss-community",
      policy: "zerant:oss:contribution · v0.1",
      threshold: 40,
      result: true,
      requestWindow: "5 minutes from issuance",
      attestationWindow: "At most 24 hours for threshold attestations",
    },
    sharedSummary: "One contribution-threshold result: true.",
    withheld: ["Exact contribution score", "Source credentials and contribution history", ...commonPrivate],
    issuerSummary: "Evaluate the pinned contribution policy and sign only the supported result.",
    verifierSummary: "Use the same generic protocol as every other scenario; OSS is one policy context, not the product identity.",
  },
  {
    id: "marketplace",
    label: "Marketplace",
    audience: "Marketplace",
    request: {
      origin: "https://market.example",
      purpose: "Confirm a service fulfillment threshold",
      issuer: "zerant:issuer:market-demo",
      claim: "reputation.threshold",
      context: "service-fulfillment",
      policy: "zerant:marketplace-fulfillment:eligibility · v0.1",
      threshold: 60,
      result: true,
      requestWindow: "5 minutes from issuance",
      attestationWindow: "At most 24 hours for threshold attestations",
    },
    sharedSummary: "One fulfillment threshold result for this marketplace context.",
    withheld: ["Exact fulfillment score", "Other buyers/sellers and orders", ...commonPrivate],
    issuerSummary: "Attest to fulfillment evidence only within the configured marketplace context.",
    verifierSummary: "Verify the threshold without importing unrelated marketplace history.",
  },
  {
    id: "organization",
    label: "Organization / role",
    audience: "Team application",
    request: {
      origin: "https://team.example",
      purpose: "Confirm one active organizational role",
      issuer: "zerant:issuer:org-demo",
      claim: "role.active",
      context: "organization-role",
      result: "maintainer",
      requestWindow: "5 minutes from issuance",
      attestationWindow: "Bounded by issuer validity",
    },
    sharedSummary: "One audience-bound active-role attestation.",
    withheld: ["Other organizations and roles", "Internal team history", ...commonPrivate],
    issuerSummary: "Use separate audience keys and attest only to the role the application needs.",
    verifierSummary: "Verify one role without turning the holder into a public directory entry.",
    zcashStatus: "FROST may later protect organizational/issuer signing keys; it is not required for holder consent.",
  },
  {
    id: "payment",
    label: "Payment receipt",
    audience: "Merchant / service",
    request: {
      origin: "https://merchant.example",
      purpose: "Confirm one invoice payment condition",
      issuer: "zerant:issuer:payment-demo",
      claim: "payment.invoice_paid",
      context: "invoice-receipt",
      result: true,
      requestWindow: "5 minutes from issuance",
      attestationWindow: "Bounded by issuer payment policy",
    },
    sharedSummary: "One paid-invoice boolean attestation. No wallet-wide data is part of the credential.",
    withheld: ["Transaction ID", "Recipient/address details", "Amount and memo", ...commonPrivate],
    issuerSummary: "Validate the expected recipient, amount, network, transaction binding and reorg policy before signing a minimal paid-invoice claim.",
    verifierSummary: "Receive the invoice result, not the payer’s wallet or transaction history.",
    zcashStatus: "Z3 capability/readiness adapter is implemented. Live payment execution is not enabled until official regtest is exercised.",
  },
];

// One public policy fixture drives both the Rust vertical vectors and browser requests.
export const scenarios: Scenario[] = sharedScenarios.map((fixture) => {
  const detail = scenarioDetails.find((item) => item.id === fixture.id)!;
  const policy = fixture.policy;
  return {
    ...detail,
    label: fixture.label,
    request: {
      ...detail.request,
      origin: `https://${fixture.id}.example`,
      purpose: fixture.purpose,
      issuer: policy.issuer_id,
      claim: "reputation.threshold",
      context: policy.context_id,
      policy: `${policy.policy_id} · v${policy.version}`,
      threshold: policy.supported_thresholds[0],
      result: true,
      attestationWindow: "At most 24 hours, bounded by source expiry and policy window",
    },
    sharedSummary: "One contextual issuer-attested eligibility threshold: true, plus required verification metadata. The exact local score and evidence are excluded.",
  };
});

export const defaultScenario = scenarios[0];

export const metadata = [
  ["Audience subject public key", "Independent key for this verifier origin · placeholder"],
  ["Credential / revocation IDs", "Fresh attestation identifiers · placeholders"],
  ["Issuer signing key / schema", "Pinned issuer authorization · zerant.credential.v0.1"],
  ["Validity / audience", "issued_at, expires_at and exact verifier origin"],
  ["Request bindings", "Request ID, request digest, challenge, nonce and origin"],
  ["Signed envelopes", "Issuer attestation JWS + holder response JWS · simulated in web UI"],
] as const;
