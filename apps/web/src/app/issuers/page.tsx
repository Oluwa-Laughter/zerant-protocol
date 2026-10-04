import type { Metadata } from "next";
import Link from "next/link";
import { fetchZerantPublic } from "@/lib/server-api";

export const metadata: Metadata = {
  title: "Trusted issuers",
  description: "Discover organizations issuing private credentials through Zerant.",
};

export const dynamic = "force-dynamic";

type IssuerDirectoryItem = {
  issuer_id: string;
  display_name: string;
  active_key_id: string | null;
  active_schema_count: number;
  revocation_version: number;
  created_at: string;
};

type IssuerDirectoryPage = {
  items: IssuerDirectoryItem[];
  next_cursor: string | null;
};

export default async function IssuersPage({
  searchParams,
}: {
  searchParams: Promise<{ cursor?: string }>;
}) {
  const { cursor } = await searchParams;
  const safeCursor = cursor && cursor.length <= 256 ? cursor : undefined;
  const query = new URLSearchParams({ limit: "24" });
  if (safeCursor) query.set("cursor", safeCursor);

  let directory: IssuerDirectoryPage = { items: [], next_cursor: null };
  let available = false;

  try {
    const response = await fetchZerantPublic(
      "/v1/public/issuers?" + query.toString(),
    );
    available = Boolean(response?.ok);
    if (response?.ok) {
      directory = (await response.json()) as IssuerDirectoryPage;
    }
  } catch {
    available = false;
  }

  return (
    <main id="main" className="issuers-page">
      <section className="issuers-hero">
        <p className="eyebrow">Trusted issuers</p>
        <h1>See who can issue trust on Zerant.</h1>
        <p>
          Explore organizations that publish credential definitions through Zerant. Public
          profiles show what they issue and whether their trust information is current—never
          who received a credential.
        </p>
      </section>

      {!available ? (
        <section className="issuers-empty">
          <h2>Issuer discovery is temporarily unavailable.</h2>
          <p className="muted">Your private credentials and account are unaffected.</p>
        </section>
      ) : directory.items.length ? (
        <>
          <section className="issuer-directory-grid" aria-label="Trusted issuers">
            {directory.items.map((issuer) => (
              <Link
                className="issuer-directory-card"
                href={"/issuers/" + encodeURIComponent(issuer.issuer_id)}
                key={issuer.issuer_id}
              >
                <div className="issuer-directory-card-top">
                  <div>
                    <span className="eyebrow">Issuer</span>
                    <h2>{issuer.display_name}</h2>
                  </div>
                  <span
                    className={
                      issuer.active_key_id
                        ? "request-status approved"
                        : "request-status denied"
                    }
                  >
                    {issuer.active_key_id ? "Active" : "Unavailable"}
                  </span>
                </div>
                <p className="muted">
                  {issuer.active_schema_count} credential type
                  {issuer.active_schema_count === 1 ? "" : "s"} available
                </p>
                <span className="text-link">View issuer →</span>
              </Link>
            ))}
          </section>

          {directory.next_cursor ? (
            <div className="issuer-directory-more">
              <Link
                className="button button-secondary"
                href={"/issuers?cursor=" + encodeURIComponent(directory.next_cursor)}
              >
                More issuers
              </Link>
            </div>
          ) : null}
        </>
      ) : (
        <section className="issuers-empty">
          <h2>No public issuers yet.</h2>
          <p className="muted">
            Issuer profiles will appear here after organizations activate them.
          </p>
        </section>
      )}
    </main>
  );
}
