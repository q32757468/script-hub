import type { ReactNode } from "react";

export type IconName =
  | "tasks"
  | "files"
  | "settings"
  | "edit"
  | "plus"
  | "play"
  | "stop"
  | "refresh"
  | "search"
  | "copy"
  | "trash"
  | "arrow"
  | "terminal"
  | "check"
  | "close"
  | "clock";

const iconPaths: Record<IconName, ReactNode> = {
  tasks: (
    <>
      <rect x="3" y="3" width="18" height="18" rx="4" />
      <path d="m9 8 4 4-4 4m6 0h2" />
    </>
  ),
  files: (
    <>
      <path d="M3 7a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v10H3Z" />
      <path d="M3 10h18" />
    </>
  ),
  settings: (
    <>
      <path d="m9 3-1 3-3 1-2 4 2 3 1 4 4 3 3-1 4-1 3-4-1-4-2-4-4-1Z" />
      <circle cx="12" cy="12" r="3" />
    </>
  ),
  edit: <path d="m16 3 5 5-12 12-6 1 1-6ZM14 5l5 5" />,
  plus: <path d="M12 5v14M5 12h14" />,
  play: <path d="m8 4 12 8-12 8Z" />,
  stop: <rect x="6" y="6" width="12" height="12" rx="2" />,
  refresh: (
    <>
      <path d="M20 10a8 8 0 1 0-1 8M20 4v6h-6" />
    </>
  ),
  search: (
    <>
      <circle cx="10" cy="10" r="6" />
      <path d="m15 15 6 6" />
    </>
  ),
  copy: (
    <>
      <rect x="8" y="8" width="12" height="13" rx="2" />
      <path d="M16 8V3H3v13h5" />
    </>
  ),
  trash: (
    <>
      <path d="M3 6h18M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7M14 10v7" />
    </>
  ),
  arrow: <path d="M5 12h14m-6-6 6 6-6 6" />,
  terminal: (
    <>
      <path d="m4 6 6 6-6 6M13 18h7" />
    </>
  ),
  check: <path d="m5 12 4 4L19 6" />,
  close: <path d="m6 6 12 12M18 6 6 18" />,
  clock: (
    <>
      <circle cx="12" cy="12" r="9" />
      <path d="M12 7v5l3 2" />
    </>
  ),
};

export function Icon({ name, size = 18 }: { name: IconName; size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {iconPaths[name]}
    </svg>
  );
}
