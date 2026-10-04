import { proxyToZerant } from "@/lib/server-api";

export const dynamic = "force-dynamic";

export async function POST(
  request: Request,
  context: { params: Promise<{ credentialId: string }> },
) {
  const { credentialId } = await context.params;
  if (!credentialId || credentialId.length > 128) {
    return Response.json({ error: "invalid credential id" }, { status: 400 });
  }
  return proxyToZerant(
    request,
    "/v1/issuer/credentials/" + encodeURIComponent(credentialId) + "/revoke",
    { method: "POST" },
  );
}
