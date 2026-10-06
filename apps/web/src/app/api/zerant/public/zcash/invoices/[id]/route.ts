import { proxyPublicToZerant } from "@/lib/server-api";

export const dynamic = "force-dynamic";

export async function GET(_request: Request, context: { params: Promise<{ id: string }> }) {
  const { id } = await context.params;
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(id)) {
    return Response.json({ error: "invalid invoice id" }, { status: 400 });
  }
  return proxyPublicToZerant(
    "/v1/public/zcash/invoices/" + encodeURIComponent(id),
    { cacheControl: "no-store" },
  );
}
