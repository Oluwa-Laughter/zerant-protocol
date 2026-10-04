import { proxyPublicToZerant } from "@/lib/server-api";

export const dynamic = "force-dynamic";

export async function GET(
  _request: Request,
  context: { params: Promise<{ issuerId: string }> },
) {
  const { issuerId } = await context.params;
  if (
    !issuerId.startsWith("zerant:issuer:") ||
    issuerId.length > 128
  ) {
    return Response.json({ error: "invalid issuer id" }, { status: 400 });
  }

  return proxyPublicToZerant(
    "/v1/public/issuers/" + encodeURIComponent(issuerId),
  );
}
