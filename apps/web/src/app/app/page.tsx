import type { Metadata } from "next";
import { ProductWorkspace } from "@/components/product-workspace";

export const metadata: Metadata = {
  title: "Workspace",
  description: "Zerant holder, issuer, verifier and Zcash integration workspace.",
};

export default function AppPage() {
  return <ProductWorkspace />;
}
