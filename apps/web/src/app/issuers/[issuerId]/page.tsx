import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { fetchZerantPublic } from "@/lib/server-api";

type PublicKey = {
  key_id: string;
  valid_from: string;
  retired_at: string | null;
  compromised_at: string | null;
};

type CredentialSchema = {
  id: string;
  display_name: string;
  description: string;
  claim_type: string;
  context: string;
  default_expiry_days: number;
  version: number;
  active: boolean;
  supersedes_schema_id: string | null;
  retired_at: string | null;
  created_at: string;
};

type IssuerMetadata = {
  issuer_id: string;
  display_name: string;
  keys: PublicKey[];
  credential_schemas: CredentialSchema[];
  revocation_version: number;
};

type RevocationPublication = {
  version: number;
  issued_at: string;
  next_update: string;
};

export async function generateMetadata({
  params,
}: {
  params: Promise<{ issuerId: string }>;
}): Promise<Metadata> {
  const { issuerId } = await params;
  try {
    const response = await fetchZerantPublic(
      "/v1/public/issuers/" + encodeURIComponent(issuerId),
    );
    if (response?.ok) {
      const issuer = (await response.json()) as IssuerMetadata;
      return {
        title: issuer.display_name,
        description: "Public Zerant issuer profile and credential definitions.",
      };
    }
  } catch {
    // Fall through to generic metadata.
  }
  return { title: "Issuer" };
}

export const dynamic = "force-dynamic";

export default async function IssuerProfilePage({
  params,
}: {
  params: Promise<{ issuerId: string }>;
}) {
  const { issuerId } = await params;
  if (!issuerId.startsWith("zerant:issuer:") || issuerId.length > 128) {
    notFound();
  }

  const metadataResponse = await fetchZerantPublic(
    "/v1/public/issuers/" + encodeURIComponent(issuerId),
  );
  if (!metadataResponse) {
    return (
      <main id="main" className="issuer-public-page">
        <section className="issuers-empty">
          <h1>Issuer information is temporarily unavailable.</h1>
        </section>
      </main>
    );
  }
  if (metadataResponse.status === 404) notFound();
  if (!metadataResponse.ok) {
    return (
      <main id="main" className="issuer-public-page">
        <section className="issuers-empty">
          <h1>Issuer information is temporarily unavailable.</h1>
        </section>
      </main>
    );
  }

  const issuer = (await metadataResponse.json()) as IssuerMetadata;
  let revocation: RevocationPublication | null = null;
  try {
    const response = await fetchZerantPublic(
      "/v1/public/issuers/" + encodeURIComponent(issuerId) + "/revocation",
    );
    if (response?.ok) {
      revocation = (await response.json()) as RevocationPublication;
    }
  } catch {
    revocation = null;
  }

  const currentSchemas = issuer.credential_schemas.filter((schema) => schema.active);
  const historicalSchemas = issuer.credential_schemas.filter((schema) => !schema.active);
  const activeKey = issuer.keys.find(
    (key) => key.retired_at === null && key.compromised_at === null,
  );

  return (
    <main id="main" className="issuer-public-page">
      <Link href="/issuers" className="text-link">← Trusted issuers</Link>

      <section className="issuer-public-hero">
        <div>
          <p className="eyebrow">Public issuer profile</p>
          <h1>{issuer.display_name}</h1>
          <p>
            This organization publishes credential definitions through Zerant. Public trust
            information never includes the people who received credentials.
          </p>
        </div>
        <span className={activeKey ? "request-status approved" : "request-status denied"}>
          {activeKey ? "Active issuer" : "Signing unavailable"}
        </span>
      </section>

      <section className="issuer-public-summary">
        <div>
          <span className="eyebrow">Credential types</span>
          <strong>{currentSchemas.length}</strong>
        </div>
        <div>
          <span className="eyebrow">Trust updates</span>
          <strong>{revocation ? "Current" : "Unavailable"}</strong>
        </div>
        <div>
          <span className="eyebrow">Issuer since</span>
          <strong>
            {issuer.keys.length
              ? new Date(issuer.keys[issuer.keys.length - 1].valid_from).toLocaleDateString()
              : "Unknown"}
          </strong>
        </div>
      </section>

      <section className="issuer-public-section">
        <div className="section-heading">
          <p className="eyebrow">Available credentials</p>
          <h2>What this issuer can attest to.</h2>
        </div>
        {currentSchemas.length ? (
          <div className="issuer-schema-grid">
            {currentSchemas.map((schema) => (
              <article className="issuer-schema-card" key={schema.id}>
                <div className="issuer-schema-card-top">
                  <h3>{schema.display_name}</h3>
                  <span className="pill">v{schema.version}</span>
                </div>
                <p>{schema.description}</p>
                <p className="small muted">
                  Applies to {schema.context} · default validity {schema.default_expiry_days} days
                </p>
              </article>
            ))}
          </div>
        ) : (
          <div className="issuers-empty">
            <p className="muted">This issuer has no active credential types right now.</p>
          </div>
        )}
      </section>

      {historicalSchemas.length ? (
        <section className="issuer-public-section">
          <div className="section-heading">
            <p className="eyebrow">Previous definitions</p>
            <h2>Historical credential versions.</h2>
          </div>
          <div className="issuer-history-list">
            {historicalSchemas.map((schema) => (
              <article key={schema.id}>
                <strong>{schema.display_name} v{schema.version}</strong>
                <span className="small muted">{schema.context}</span>
              </article>
            ))}
          </div>
        </section>
      ) : null}
    </main>
  );
}
