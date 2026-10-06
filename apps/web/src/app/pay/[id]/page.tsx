import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { PublicZcashInvoice, type PublicInvoice } from "@/components/public-zcash-invoice";
import { fetchZerantPublic } from "@/lib/server-api";

export const metadata: Metadata = {
  title: "Zcash invoice",
  description: "Review an exact Zcash payment request shared through Zerant.",
};

export const dynamic = "force-dynamic";

export default async function PublicInvoicePage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(id)) notFound();
  const response = await fetchZerantPublic("/v1/public/zcash/invoices/" + encodeURIComponent(id));
  if (!response || response.status === 404) notFound();
  if (!response.ok) throw new Error("Invoice temporarily unavailable");
  const invoice = (await response.json()) as PublicInvoice;
  return <PublicZcashInvoice invoice={invoice} />;
}
