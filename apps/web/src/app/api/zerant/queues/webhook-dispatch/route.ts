import { zerantQueue } from "@/lib/vercel-queue";
import { dispatchWebhookRequestToZerant } from "@/lib/server-api";
import {
  isWebhookQueueMessage,
  webhookQueueRetrySeconds,
  type WebhookQueueMessage,
} from "@/lib/webhook-queue";

const INVALID_MESSAGE = "invalid_webhook_queue_message";

export const POST = zerantQueue.handleCallback<WebhookQueueMessage>(
  async (message) => {
    if (!isWebhookQueueMessage(message)) {
      throw new Error(INVALID_MESSAGE);
    }

    const summary = await dispatchWebhookRequestToZerant(message.requestId);
    if (!summary.finalized) {
      throw new Error("verification_not_finalized");
    }
    if (summary.pending > 0) {
      throw new Error("webhook_delivery_pending");
    }
  },
  {
    visibilityTimeoutSeconds: 60,
    retry: (error, metadata) => {
      if (error instanceof Error && error.message === INVALID_MESSAGE) {
        return { acknowledge: true };
      }
      if (error instanceof Error && error.message === "verification_not_finalized") {
        return { afterSeconds: 5 };
      }
      return { afterSeconds: webhookQueueRetrySeconds(metadata.deliveryCount) };
    },
  },
);
