import { proxyToZerant } from "@/lib/server-api";

export const dynamic = "force-dynamic";

export async function POST(
  request: Request,
  context: { params: Promise<{ schemaId: string }> },
) {
  const { schemaId } = await context.params;
  if (!/^[0-9a-fA-F-]{36}$/.test(schemaId)) {
    return Response.json({ error: "invalid schema id" }, { status: 400 });
  }

  return proxyToZerant(
    request,
    "/v1/issuer/schemas/" + encodeURIComponent(schemaId) + "/deactivate",
    { method: "POST" },
  );
}
