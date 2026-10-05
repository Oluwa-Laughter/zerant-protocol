import { proxyToZerant } from "@/lib/server-api";

export const dynamic = "force-dynamic";

export async function POST(request: Request, context: { params: Promise<{ id: string }> }) {
  const { id } = await context.params;
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(id)) {
    return Response.json({ error: "invalid payment id" }, { status: 400 });
  }
  return proxyToZerant(
    request,
    "/v1/zcash/payments/" + encodeURIComponent(id) + "/observe",
    { method: "POST" },
  );
}
