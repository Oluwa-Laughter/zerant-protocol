import type { Metadata } from "next";
import { ZcashWorkspace } from "@/components/zcash-workspace";
import { loadZcashWorkspaceData } from "@/app/zcash/data";

export const metadata: Metadata = {
  title: "Zcash",
  description: "Private Zcash actions, payment review, and network readiness inside Zerant.",
};

export const dynamic = "force-dynamic";

export default async function ZcashPage() {
  const data = await loadZcashWorkspaceData();
  return <ZcashWorkspace {...data} activeSection="overview" />;
}
