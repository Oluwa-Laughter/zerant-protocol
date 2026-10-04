import { proxyToZerant } from "@/lib/server-api";

export const dynamic = "force-dynamic";

export async function DELETE(
  request: Request,
  context: { params: Promise<{ id: string }> },
) {
  const { id } = await context.params;
  if (!/^[0-9a-fA-F-]{36}$/.test(id)) {
    return Response.json({ error: "invalid credential id" }, { status: 400 });
  }
  return proxyToZerant(request, "/v1/credentials/" + encodeURIComponent(id), { method: "DELETE" });
}
