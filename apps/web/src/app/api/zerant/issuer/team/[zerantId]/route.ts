import { proxyToZerant } from "@/lib/server-api";
export const dynamic = "force-dynamic";
export async function DELETE(
  request: Request,
  context: { params: Promise<{ zerantId: string }> },
) {
  const { zerantId } = await context.params;
  if (!/^zr_[0-9a-f]{24}$/.test(zerantId)) {
    return Response.json({ error: "invalid Zerant ID" }, { status: 400 });
  }
  return proxyToZerant(
    request,
    "/v1/issuer/team/" + encodeURIComponent(zerantId),
    { method: "DELETE" },
  );
}
