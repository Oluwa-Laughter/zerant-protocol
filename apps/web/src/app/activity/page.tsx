import type { Metadata } from "next";
import { cookies } from "next/headers";
import {
  ActivityTimeline,
  type ActivityPageData,
} from "@/components/activity-timeline";
import { fetchZerantBackend } from "@/lib/server-api";

export const metadata: Metadata = {
  title: "Activity",
  description: "Review credential and verification activity in Zerant.",
};

export const dynamic = "force-dynamic";

export default async function ActivityPage() {
  const cookieStore = await cookies();
  const cookieHeader = cookieStore.toString();

  let page: ActivityPageData = { items: [], next_cursor: null };

  try {
    const response = await fetchZerantBackend("/v1/activity?limit=20", cookieHeader);
    if (response?.ok) {
      page = (await response.json()) as ActivityPageData;
    }
  } catch {
    page = { items: [], next_cursor: null };
  }

  return <ActivityTimeline initialPage={page} />;
}
