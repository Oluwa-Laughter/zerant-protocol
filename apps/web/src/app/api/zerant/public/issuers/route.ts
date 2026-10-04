import { proxyPublicToZerant } from "@/lib/server-api";

export const dynamic = "force-dynamic";

export async function GET(request: Request) {
  const url = new URL(request.url);
  const limit = url.searchParams.get("limit");
  const cursor = url.searchParams.get("cursor");
  const params = new URLSearchParams();

  if (limit) {
    const parsed = Number.parseInt(limit, 10);
    if (!Number.isInteger(parsed) || parsed < 1 || parsed > 100) {
      return Response.json({ error: "invalid limit" }, { status: 400 });
    }
    params.set("limit", String(parsed));
  }

  if (cursor) {
    if (cursor.length > 256) {
      return Response.json({ error: "invalid cursor" }, { status: 400 });
    }
    params.set("cursor", cursor);
  }

  const query = params.size ? "?" + params.toString() : "";
  return proxyPublicToZerant("/v1/public/issuers" + query);
}
