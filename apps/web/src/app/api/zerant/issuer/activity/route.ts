import { proxyToZerant } from "@/lib/server-api";

export const dynamic = "force-dynamic";

export async function GET(request: Request) {
  const url = new URL(request.url);
  const query = new URLSearchParams();
  const cursor = url.searchParams.get("cursor");
  const limit = url.searchParams.get("limit");
  if (cursor) query.set("cursor", cursor);
  if (limit) query.set("limit", limit);
  const suffix = query.size ? "?" + query.toString() : "";
  return proxyToZerant(request, "/v1/issuer/activity" + suffix, { method: "GET" });
}
