import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { IssuerWorkspace } from "@/components/issuer-workspace";
import { loadIssuerWorkspaceData } from "@/app/issuer/data";

export const dynamic = "force-dynamic";

const sections = ["team", "schemas", "payouts", "security", "activity", "issue"] as const;
type IssuerSection = (typeof sections)[number];

export const metadata: Metadata = {
  title: "Issuer workspace",
  description: "Manage one issuer workspace section at a time.",
};

export default async function IssuerSectionPage({ params }: { params: Promise<{ section: string }> }) {
  const { section } = await params;
  if (!sections.includes(section as IssuerSection)) notFound();
  const data = await loadIssuerWorkspaceData();
  return <IssuerWorkspace
    authenticated={data.authenticated}
    backendAvailable={data.backendAvailable}
    currentZerantId={data.currentZerantId}
    initialProfile={data.profile}
    initialIssued={data.issued}
    initialSchemas={data.schemas}
    initialKeys={data.keys}
    initialTeam={data.team}
    initialTeamInvitations={data.teamInvitations}
    initialMyInvitations={data.myInvitations}
    initialActivity={data.activity}
    activeSection={section as IssuerSection}
  />;
}
