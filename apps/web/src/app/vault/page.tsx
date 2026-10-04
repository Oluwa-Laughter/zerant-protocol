import type { Metadata } from "next";
import { HolderVault } from "@/components/holder-vault";

export const metadata: Metadata = {
  title: "Local holder vault",
  description: "Local encrypted holder storage using browser Web Crypto and IndexedDB.",
};

export default function VaultPage() {
  return <HolderVault />;
}
