import { proxyToZerant } from "@/lib/server-api";

export const dynamic = "force-dynamic";

export async function GET(
  request: Request,
  context: { params: Promise<{ id: string }> },
) {
  const { id } = await context.params;
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(id)) {
    return Response.json({ error: "invalid request id" }, { status: 400 });
  }
  return proxyToZerant(
    request,
    "/v1/verifier/requests/" + encodeURIComponent(id) + "/proof",
    { method: "GET" },
  );
}
