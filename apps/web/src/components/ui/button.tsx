import type { ButtonHTMLAttributes, Ref } from "react";
import { cn } from "@/lib/utils";
export function Button({ className, variant = "primary", ref, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: "primary" | "secondary"; ref?: Ref<HTMLButtonElement> }) {
  return <button ref={ref} className={cn("button", variant === "secondary" && "button-secondary", className)} {...props} />;
}
