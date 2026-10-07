import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { ZcashWorkspace } from "@/components/zcash-workspace";
import { loadZcashWorkspaceData } from "@/app/zcash/data";

export const dynamic = "force-dynamic";

const sections = ["payments", "invoices", "payouts", "wallet", "address"] as const;
type ZcashSection = (typeof sections)[number];

export const metadata: Metadata = {
  title: "Zcash workspace",
  description: "Use one Zcash workspace tool at a time.",
};

export default async function ZcashSectionPage({ params }: { params: Promise<{ section: string }> }) {
  const { section } = await params;
  if (!sections.includes(section as ZcashSection)) notFound();
  const data = await loadZcashWorkspaceData();
  return <ZcashWorkspace {...data} activeSection={section as ZcashSection} />;
}
