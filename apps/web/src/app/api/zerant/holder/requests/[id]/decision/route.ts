import { zerantQueue } from "@/lib/vercel-queue";
import {
  dispatchWebhookRequestToZerant,
  proxyToZerant,
} from "@/lib/server-api";
import {
  WEBHOOK_QUEUE_RETENTION_SECONDS,
  WEBHOOK_QUEUE_TOPIC,
} from "@/lib/webhook-queue";

export const dynamic = "force-dynamic";

export async function POST(
  request: Request,
  context: { params: Promise<{ id: string }> },
) {
  const { id } = await context.params;
  if (!/^[0-9a-fA-F-]{36}$/.test(id)) {
    return Response.json({ error: "invalid request id" }, { status: 400 });
  }

  const onVercel = Boolean(process.env.VERCEL);
  if (onVercel) {
    try {
      await zerantQueue.send(
        WEBHOOK_QUEUE_TOPIC,
        { requestId: id },
        {
          idempotencyKey: "verification:" + id,
          retentionSeconds: WEBHOOK_QUEUE_RETENTION_SECONDS,
          delaySeconds: 2,
        },
      );
    } catch {
      return Response.json(
        { error: "verification delivery queue unavailable" },
        { status: 503 },
      );
    }
  }

  const response = await proxyToZerant(
    request,
    "/v1/holder/requests/" + encodeURIComponent(id) + "/decision",
    { method: "POST" },
  );

  if (!onVercel && response.ok) {
    try {
      await dispatchWebhookRequestToZerant(id);
    } catch {
      // Local development has no durable Vercel Queue. The decision remains valid;
      // webhook delivery can be retried after the endpoint is available.
    }
  }

  return response;
}
