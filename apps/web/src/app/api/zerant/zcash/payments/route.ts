import { proxyToZerant } from "@/lib/server-api";

export const dynamic = "force-dynamic";

export async function GET(request: Request) {
  const cursor = new URL(request.url).searchParams.get("cursor");
  if (cursor && (cursor.length > 128 || !/^[A-Za-z0-9_-]+$/.test(cursor))) {
    return Response.json({ error: "invalid cursor" }, { status: 400 });
  }
  const path = "/v1/zcash/payments" + (cursor ? "?cursor=" + encodeURIComponent(cursor) : "");
  return proxyToZerant(request, path, { method: "GET" });
}

export async function POST(request: Request) {
  return proxyToZerant(request, "/v1/zcash/payments", { method: "POST" });
}
