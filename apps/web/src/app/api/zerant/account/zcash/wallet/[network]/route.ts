import { proxyToZerant } from "@/lib/server-api";

export const dynamic = "force-dynamic";

export async function DELETE(request: Request, context: { params: Promise<{ network: string }> }) {
  const { network } = await context.params;
  if (network !== "testnet" && network !== "mainnet") {
    return Response.json({ error: "invalid request" }, { status: 400 });
  }
  return proxyToZerant(request, `/v1/account/zcash/wallet/${network}`, { method: "DELETE" });
}
