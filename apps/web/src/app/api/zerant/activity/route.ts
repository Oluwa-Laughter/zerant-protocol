import { proxyToZerant } from "@/lib/server-api";

export const dynamic = "force-dynamic";

export async function GET(request: Request) {
  const url = new URL(request.url);
  const query = url.searchParams.toString();
  return proxyToZerant(
    request,
    "/v1/activity" + (query ? "?" + query : ""),
    { method: "GET" },
  );
}
