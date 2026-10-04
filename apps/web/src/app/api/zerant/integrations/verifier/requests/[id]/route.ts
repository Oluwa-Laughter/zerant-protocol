import { proxyIntegrationToZerant } from "@/lib/server-api";

export const dynamic = "force-dynamic";

export async function GET(
  request: Request,
  context: { params: Promise<{ id: string }> },
) {
  const { id } = await context.params;
  if (!/^[0-9a-fA-F-]{36}$/.test(id)) {
    return Response.json({ error: "invalid request id" }, { status: 400 });
  }
  return proxyIntegrationToZerant(
    request,
    "/v1/integrations/verifier/requests/" + encodeURIComponent(id),
    { method: "GET" },
  );
}
