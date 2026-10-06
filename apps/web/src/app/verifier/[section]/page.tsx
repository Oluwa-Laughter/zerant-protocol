import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { VerifierWorkspace } from "@/components/verifier-workspace";
import { loadVerifierWorkspaceData } from "@/app/verifier/data";

export const dynamic = "force-dynamic";

const sections = ["requests", "integrations", "security"] as const;
type VerifierSection = (typeof sections)[number];

export const metadata: Metadata = {
  title: "Verifier workspace",
  description: "Manage one verifier workspace section at a time.",
};

export default async function VerifierSectionPage({ params }: { params: Promise<{ section: string }> }) {
  const { section } = await params;
  if (!sections.includes(section as VerifierSection)) notFound();
  const data = await loadVerifierWorkspaceData();
  return <VerifierWorkspace
    authenticated={data.authenticated}
    backendAvailable={data.backendAvailable}
    initialProfile={data.profile}
    issuers={data.issuers}
    initialRequests={data.requests}
    initialKeys={data.keys}
    initialApiKeys={data.apiKeys}
    initialWebhooks={data.webhooks}
    activeSection={section as VerifierSection}
  />;
}
