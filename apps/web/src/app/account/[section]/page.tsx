import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { AccountSettings } from "@/components/account-settings";
import { loadAccountData } from "@/app/account/data";

export const dynamic = "force-dynamic";

const sections = ["access", "sessions", "data"] as const;
type AccountSection = (typeof sections)[number];

export const metadata: Metadata = {
  title: "Account settings",
  description: "Manage one account settings section at a time.",
};

export default async function AccountSectionPage({ params }: { params: Promise<{ section: string }> }) {
  const { section } = await params;
  if (!sections.includes(section as AccountSection)) notFound();
  const data = await loadAccountData();
  return <AccountSettings
    authenticated={data.authenticated}
    backendAvailable={data.backendAvailable}
    summary={data.summary}
    initialPasskeys={data.passkeys}
    initialSessions={data.sessions}
    initialZcashMethods={data.zcashMethods}
    activeSection={section as AccountSection}
  />;
}
