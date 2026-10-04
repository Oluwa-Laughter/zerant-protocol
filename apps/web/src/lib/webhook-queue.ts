export const WEBHOOK_QUEUE_TOPIC = "zerant-webhook-dispatch";
export const WEBHOOK_QUEUE_RETENTION_SECONDS = 7 * 24 * 60 * 60;

export type WebhookQueueMessage = {
  requestId: string;
};

const RETRY_SECONDS = [60, 300, 900, 3600, 21_600, 43_200, 86_400] as const;

export function webhookQueueRetrySeconds(deliveryCount: number): number {
  const index = Math.max(0, Math.min(deliveryCount - 1, RETRY_SECONDS.length - 1));
  return RETRY_SECONDS[index];
}

export function isWebhookQueueMessage(value: unknown): value is WebhookQueueMessage {
  if (!value || typeof value !== "object") return false;
  const requestId = (value as { requestId?: unknown }).requestId;
  return (
    typeof requestId === "string" &&
    /^[0-9a-fA-F-]{36}$/.test(requestId)
  );
}
