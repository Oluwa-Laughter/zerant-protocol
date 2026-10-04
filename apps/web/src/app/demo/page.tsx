import type { Metadata } from "next";
import { DemoDashboard } from "@/components/demo-dashboard";

export const metadata: Metadata = { title: "Protocol playground" };

export default function DemoPage() {
  return <main id="main"><DemoDashboard /></main>;
}
