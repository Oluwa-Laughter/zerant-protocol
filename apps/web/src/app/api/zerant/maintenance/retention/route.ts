import { isAuthorizedCronRequest } from "@/lib/cron-auth";
import { runRetentionMaintenanceToZerant } from "@/lib/server-api";

export const dynamic = "force-dynamic";

export async function GET(request: Request) {
  const secret = process.env.CRON_SECRET;
  if (!secret) {
    return Response.json({ error: "maintenance unavailable" }, { status: 503 });
  }
  if (!isAuthorizedCronRequest(request.headers.get("authorization"), secret)) {
    return Response.json({ error: "unauthorized" }, { status: 401 });
  }

  try {
    const summary = await runRetentionMaintenanceToZerant();
    return Response.json(summary, {
      headers: { "cache-control": "no-store" },
    });
  } catch {
    return Response.json({ error: "maintenance failed" }, { status: 503 });
  }
}
