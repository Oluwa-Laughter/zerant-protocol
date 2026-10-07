"use client";

import { useEffect, useId, useRef, useState } from "react";
import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";

export type MobileMenuGroup = {
  label: string;
  items: { href: string; label: string }[];
};

export function MobileMenu({ groups, label = "Open navigation", signOut = false }: {
  groups: MobileMenuGroup[];
  label?: string;
  signOut?: boolean;
}) {
  const pathname = usePathname();
  const router = useRouter();
  const [openedPath, setOpenedPath] = useState<string | null>(null);
  const open = openedPath === pathname;
  const panelId = useId();
  const triggerRef = useRef<HTMLButtonElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const previousOverflow = document.body.style.overflow;
    const trigger = triggerRef.current;
    document.body.style.overflow = "hidden";
    panelRef.current?.querySelector<HTMLButtonElement>("button")?.focus();
    return () => {
      document.body.style.overflow = previousOverflow;
      trigger?.focus();
    };
  }, [open]);

  function handleKeyDown(event: React.KeyboardEvent<HTMLDivElement>) {
    if (event.key === "Escape") {
      event.preventDefault();
      setOpenedPath(null);
      return;
    }
    if (event.key !== "Tab") return;
    const focusables = [...(panelRef.current?.querySelectorAll<HTMLElement>(
      'a[href], button:not([disabled])',
    ) ?? [])];
    const first = focusables[0];
    const last = focusables.at(-1);
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last?.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first?.focus();
    }
  }

  async function handleSignOut() {
    const response = await fetch("/api/zerant/session", { method: "DELETE", credentials: "same-origin" });
    if (response.ok || response.status === 401) {
      setOpenedPath(null);
      router.push("/");
      router.refresh();
    }
  }

  return <>
    <button
      ref={triggerRef}
      type="button"
      className="mobile-menu-trigger"
      aria-label={open ? "Close navigation" : label}
      aria-expanded={open}
      aria-controls={panelId}
      onClick={() => setOpenedPath((current) => current === pathname ? null : pathname)}
    ><span aria-hidden="true" /><span aria-hidden="true" /><span aria-hidden="true" /></button>
    {open ? <div className="mobile-menu-layer">
      <button type="button" className="mobile-menu-scrim" aria-label="Close navigation" onClick={() => setOpenedPath(null)} />
      <div
        id={panelId}
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-label="Navigation"
        className="mobile-menu-panel"
        onKeyDown={handleKeyDown}
      >
        <div className="mobile-menu-heading"><strong>Zerant</strong><button type="button" onClick={() => setOpenedPath(null)} aria-label="Close navigation">×</button></div>
        <nav aria-label="Mobile navigation">
          {groups.map((group) => <div className="mobile-menu-group" key={group.label}>
            <p>{group.label}</p>
            {group.items.map((item) => {
              const active = pathname === item.href || (item.href !== "/" && pathname.startsWith(item.href + "/"));
              return <Link key={item.href} href={item.href} aria-current={active ? "page" : undefined} onClick={() => setOpenedPath(null)}>{item.label}</Link>;
            })}
          </div>)}
          {signOut ? <button type="button" className="mobile-menu-signout" onClick={() => void handleSignOut()}>Sign out</button> : null}
        </nav>
      </div>
    </div> : null}
  </>;
}
