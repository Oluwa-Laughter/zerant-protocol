import Link from "next/link";

export type WorkspaceSection = {
  href: string;
  label: string;
};

export function WorkspaceSectionNav({
  items,
  activeHref,
}: {
  items: WorkspaceSection[];
  activeHref: string;
}) {
  return (
    <nav className="workspace-section-nav" aria-label="Workspace section">
      {items.map((item) => {
        const active = item.href === activeHref;
        return (
          <Link
            key={item.href}
            href={item.href}
            className={active ? "workspace-section-link active" : "workspace-section-link"}
            aria-current={active ? "page" : undefined}
          >
            {item.label}
          </Link>
        );
      })}
    </nav>
  );
}
