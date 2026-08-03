export type NavigationIconName =
  | "terminal"
  | "groups"
  | "hosts"
  | "keys"
  | "routes"
  | "knownHosts"
  | "snippets"
  | "appearance";

export function NavigationIcon({ name }: { name: NavigationIconName }) {
  const paths: Record<NavigationIconName, string> = {
    terminal: "M4 5h16v14H4zM7.5 9l3 3-3 3M12.5 15H17",
    groups: "M5 5h6v5H5zM13 14h6v5h-6zM8 10v2a2 2 0 0 0 2 2h3",
    hosts: "M4 5.5h16v11H4zM8 19h8M12 16.5V19",
    keys: "M15.5 7.5a4 4 0 1 1-3.7 5.5L4 20.8V17h3v-3h3l1.8-1.8",
    routes: "M6 5.5h4v4H6zM14 14.5h4v4h-4zM10 7.5h3a3 3 0 0 1 3 3v4",
    knownHosts:
      "M12 3.5 19 6v5.5c0 4.2-2.8 7.4-7 9-4.2-1.6-7-4.8-7-9V6l7-2.5Zm-3 8 2 2 4-4",
    snippets: "M5 4.5h14v15H5zM8 8h8M8 12h5M8 16h7",
    appearance: "M5 18 10.5 5h3L19 18M7.2 13h9.6M18 5.5h2M19 4.5v2",
  };

  return (
    <svg aria-hidden="true" fill="none" viewBox="0 0 24 24">
      <path
        d={paths[name]}
        stroke="currentColor"
        strokeLinecap="round"
        strokeWidth="1.7"
      />
    </svg>
  );
}

export function LockIcon() {
  return (
    <svg aria-hidden="true" fill="none" viewBox="0 0 24 24">
      <path
        d="M7.5 10V7.8a4.5 4.5 0 0 1 9 0V10m-10 0h11a1 1 0 0 1 1 1v8h-13v-8a1 1 0 0 1 1-1Z"
        stroke="currentColor"
        strokeLinecap="round"
        strokeWidth="1.7"
      />
    </svg>
  );
}

export function FingerprintIcon() {
  return (
    <svg aria-hidden="true" fill="none" viewBox="0 0 24 24">
      <path
        d="M7.2 9.2A5.2 5.2 0 0 1 17 11.6m-11.7 2A6.8 6.8 0 0 1 18.8 12m-11 4.8c1.7-1.4 2.4-3.2 2.4-5.3a1.8 1.8 0 0 1 3.6 0c0 3.5-1.2 6.3-3.5 8.4m4.7-1.2c1.4-2.1 2-4.5 2-7.2A5 5 0 0 0 7 11.5c0 .8-.1 1.6-.3 2.3m10.7 5.1c1.2-2.3 1.8-4.8 1.8-7.4A7.2 7.2 0 0 0 5 10.1"
        stroke="currentColor"
        strokeLinecap="round"
        strokeWidth="1.7"
      />
    </svg>
  );
}

export function ShieldIcon() {
  return (
    <svg aria-hidden="true" fill="none" viewBox="0 0 24 24">
      <path
        d="M12 3.5 19 6v5.5c0 4.2-2.8 7.4-7 9-4.2-1.6-7-4.8-7-9V6l7-2.5Zm-3 8 2 2 4-4"
        stroke="currentColor"
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth="1.7"
      />
    </svg>
  );
}

export function WarningIcon() {
  return (
    <svg aria-hidden="true" fill="none" viewBox="0 0 24 24">
      <path
        d="M12 4 21 20H3L12 4Zm0 5v5m0 3.2v.1"
        stroke="currentColor"
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth="1.7"
      />
    </svg>
  );
}
