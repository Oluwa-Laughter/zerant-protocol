import "server-only";

import { QueueClient, type VercelRegion } from "@vercel/queue";

const region = (process.env.VERCEL_REGION ?? "iad1") as VercelRegion;

export const zerantQueue = new QueueClient({ region });
