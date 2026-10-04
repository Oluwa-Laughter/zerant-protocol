import { proxyToZerant } from "@/lib/server-api";

export const dynamic = "force-dynamic";

export async function POST(
  request: Request,
  context: { params: Promise<{ id: string }> },
) {
  const { id } = await context.params;
  if (!/^[0-9a-fA-F-]{36}$/.test(id)) {
    return Response.json({ error: "invalid key id" }, { status: 400 });
  }
  return proxyToZerant(
    request,
    "/v1/verifier/api-keys/" + encodeURIComponent(id) + "/revoke",
    { method: "POST" },
  );
}
