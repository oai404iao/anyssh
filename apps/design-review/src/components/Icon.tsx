import type { SVGProps } from "react";

export type IconName =
  | "android"
  | "appearance"
  | "arrow"
  | "back"
  | "check"
  | "chevron"
  | "compare"
  | "copy"
  | "credentials"
  | "desktop"
  | "edit"
  | "fingerprint"
  | "forwarding"
  | "grid"
  | "host"
  | "info"
  | "key"
  | "lock"
  | "menu"
  | "moon"
  | "more"
  | "plus"
  | "search"
  | "security"
  | "sessions"
  | "snippet"
  | "sun"
  | "terminal"
  | "warning";

interface IconProps extends SVGProps<SVGSVGElement> {
  name: IconName;
}

export function Icon({ name, ...props }: IconProps) {
  return (
    <svg aria-hidden="true" fill="none" viewBox="0 0 24 24" {...props}>
      {iconPath(name)}
    </svg>
  );
}

function iconPath(name: IconName) {
  switch (name) {
    case "android":
      return (
        <>
          <path d="M7 8.5h10v8H7z" stroke="currentColor" strokeWidth="1.7" />
          <path
            d="M8.5 8.5a3.5 3.5 0 0 1 7 0M9 5 7.8 3.5M15 5l1.2-1.5M9.5 12h.01M14.5 12h.01M9 16.5V19M15 16.5V19"
            stroke="currentColor"
            strokeLinecap="round"
            strokeWidth="1.7"
          />
        </>
      );
    case "appearance":
      return (
        <>
          <path
            d="M12 3.5a8.5 8.5 0 1 0 0 17h1.2a1.8 1.8 0 0 0 .3-3.6l-.7-.1a1.6 1.6 0 0 1 .3-3.2h2.2A5.7 5.7 0 0 0 21 8c0-2.5-3.8-4.5-9-4.5Z"
            stroke="currentColor"
            strokeLinejoin="round"
            strokeWidth="1.7"
          />
          <path
            d="M7.5 9h.01M10 6.8h.01M14 6.7h.01M17 9h.01"
            stroke="currentColor"
            strokeLinecap="round"
            strokeWidth="2.2"
          />
        </>
      );
    case "arrow":
      return (
        <path
          d="M5 12h13m-5-5 5 5-5 5"
          stroke="currentColor"
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth="1.8"
        />
      );
    case "back":
      return (
        <path
          d="m14.5 6-6 6 6 6"
          stroke="currentColor"
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth="1.8"
        />
      );
    case "check":
      return (
        <path
          d="m5 12.5 4.2 4.2L19 7"
          stroke="currentColor"
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth="1.9"
        />
      );
    case "chevron":
      return (
        <path
          d="m9 6 6 6-6 6"
          stroke="currentColor"
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth="1.8"
        />
      );
    case "compare":
      return (
        <>
          <rect
            height="13"
            rx="1.5"
            stroke="currentColor"
            strokeWidth="1.7"
            width="11"
            x="2.5"
            y="5.5"
          />
          <rect
            height="16"
            rx="2"
            stroke="currentColor"
            strokeWidth="1.7"
            width="7"
            x="15"
            y="4"
          />
        </>
      );
    case "copy":
      return (
        <>
          <rect
            height="12"
            rx="2"
            stroke="currentColor"
            strokeWidth="1.7"
            width="11"
            x="8"
            y="8"
          />
          <path
            d="M6 16H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"
            stroke="currentColor"
            strokeLinecap="round"
            strokeWidth="1.7"
          />
        </>
      );
    case "credentials":
    case "key":
      return (
        <>
          <circle
            cx="8"
            cy="12"
            r="3.5"
            stroke="currentColor"
            strokeWidth="1.7"
          />
          <path
            d="M11.5 12H21m-3 0v3m-3-3v2"
            stroke="currentColor"
            strokeLinecap="round"
            strokeWidth="1.7"
          />
        </>
      );
    case "desktop":
      return (
        <>
          <rect
            height="12"
            rx="2"
            stroke="currentColor"
            strokeWidth="1.7"
            width="18"
            x="3"
            y="4"
          />
          <path
            d="M9 20h6m-3-4v4"
            stroke="currentColor"
            strokeLinecap="round"
            strokeWidth="1.7"
          />
        </>
      );
    case "edit":
      return (
        <>
          <path
            d="m14.5 5.5 4 4M5 19l1-4L16.5 4.5a1.4 1.4 0 0 1 2 0l1 1a1.4 1.4 0 0 1 0 2L9 18l-4 1Z"
            stroke="currentColor"
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth="1.7"
          />
        </>
      );
    case "fingerprint":
      return (
        <>
          <path
            d="M7 10a5 5 0 0 1 10 0c0 4.5-1.6 8-4 10M9 20c2-3 2-6 2-9a1 1 0 0 1 2 0c0 2.6-.4 5.2-1.5 7.5"
            stroke="currentColor"
            strokeLinecap="round"
            strokeWidth="1.6"
          />
          <path
            d="M4.5 14c.3-1.3.5-2.6.5-4a7 7 0 0 1 14 0c0 2.1-.3 4-.8 5.7M6 18c1.1-2.4 1-5.3 1-8"
            stroke="currentColor"
            strokeLinecap="round"
            strokeWidth="1.6"
          />
        </>
      );
    case "forwarding":
      return (
        <>
          <path
            d="M5 7h11m0 0-3-3m3 3-3 3M19 17H8m0 0 3 3m-3-3 3-3"
            stroke="currentColor"
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth="1.7"
          />
        </>
      );
    case "grid":
      return (
        <>
          <rect
            height="7"
            rx="1.5"
            stroke="currentColor"
            strokeWidth="1.7"
            width="7"
            x="3"
            y="3"
          />
          <rect
            height="7"
            rx="1.5"
            stroke="currentColor"
            strokeWidth="1.7"
            width="7"
            x="14"
            y="3"
          />
          <rect
            height="7"
            rx="1.5"
            stroke="currentColor"
            strokeWidth="1.7"
            width="7"
            x="3"
            y="14"
          />
          <rect
            height="7"
            rx="1.5"
            stroke="currentColor"
            strokeWidth="1.7"
            width="7"
            x="14"
            y="14"
          />
        </>
      );
    case "host":
      return (
        <>
          <rect
            height="7"
            rx="1.5"
            stroke="currentColor"
            strokeWidth="1.7"
            width="17"
            x="3.5"
            y="3.5"
          />
          <rect
            height="7"
            rx="1.5"
            stroke="currentColor"
            strokeWidth="1.7"
            width="17"
            x="3.5"
            y="13.5"
          />
          <path
            d="M7 7h.01M7 17h.01M10 7h6M10 17h6"
            stroke="currentColor"
            strokeLinecap="round"
            strokeWidth="1.7"
          />
        </>
      );
    case "info":
      return (
        <>
          <circle
            cx="12"
            cy="12"
            r="9"
            stroke="currentColor"
            strokeWidth="1.7"
          />
          <path
            d="M12 10v6M12 7h.01"
            stroke="currentColor"
            strokeLinecap="round"
            strokeWidth="1.9"
          />
        </>
      );
    case "lock":
      return (
        <>
          <rect
            height="11"
            rx="2"
            stroke="currentColor"
            strokeWidth="1.7"
            width="14"
            x="5"
            y="10"
          />
          <path
            d="M8 10V7a4 4 0 0 1 8 0v3"
            stroke="currentColor"
            strokeLinecap="round"
            strokeWidth="1.7"
          />
        </>
      );
    case "menu":
      return (
        <path
          d="M4 7h16M4 12h16M4 17h16"
          stroke="currentColor"
          strokeLinecap="round"
          strokeWidth="1.8"
        />
      );
    case "moon":
      return (
        <path
          d="M20 15.5A8 8 0 0 1 8.5 4 8.5 8.5 0 1 0 20 15.5Z"
          stroke="currentColor"
          strokeLinejoin="round"
          strokeWidth="1.7"
        />
      );
    case "more":
      return (
        <path
          d="M6 12h.01M12 12h.01M18 12h.01"
          stroke="currentColor"
          strokeLinecap="round"
          strokeWidth="2.4"
        />
      );
    case "plus":
      return (
        <path
          d="M12 5v14M5 12h14"
          stroke="currentColor"
          strokeLinecap="round"
          strokeWidth="1.8"
        />
      );
    case "search":
      return (
        <>
          <circle
            cx="10.5"
            cy="10.5"
            r="6.5"
            stroke="currentColor"
            strokeWidth="1.7"
          />
          <path
            d="m15.5 15.5 4.5 4.5"
            stroke="currentColor"
            strokeLinecap="round"
            strokeWidth="1.7"
          />
        </>
      );
    case "security":
      return (
        <path
          d="M12 3 4.5 6v5.3c0 4.3 3 7.9 7.5 9.7 4.5-1.8 7.5-5.4 7.5-9.7V6L12 3Z"
          stroke="currentColor"
          strokeLinejoin="round"
          strokeWidth="1.7"
        />
      );
    case "sessions":
      return (
        <>
          <rect
            height="13"
            rx="2"
            stroke="currentColor"
            strokeWidth="1.7"
            width="16"
            x="4"
            y="4"
          />
          <path
            d="m7.5 9 2.5 2-2.5 2M12 13h4M8 20h8"
            stroke="currentColor"
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth="1.7"
          />
        </>
      );
    case "snippet":
      return (
        <>
          <path
            d="m9 8-4 4 4 4m6-8 4 4-4 4M13 5l-2 14"
            stroke="currentColor"
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth="1.7"
          />
        </>
      );
    case "sun":
      return (
        <>
          <circle
            cx="12"
            cy="12"
            r="3.5"
            stroke="currentColor"
            strokeWidth="1.7"
          />
          <path
            d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4m11.4 11.4 1.4 1.4M2 12h2m16 0h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"
            stroke="currentColor"
            strokeLinecap="round"
            strokeWidth="1.7"
          />
        </>
      );
    case "terminal":
      return (
        <>
          <rect
            height="16"
            rx="2"
            stroke="currentColor"
            strokeWidth="1.7"
            width="19"
            x="2.5"
            y="4"
          />
          <path
            d="m6.5 9 3 3-3 3M12 15h5"
            stroke="currentColor"
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth="1.7"
          />
        </>
      );
    case "warning":
      return (
        <>
          <path
            d="M12 3 2.8 19h18.4L12 3Z"
            stroke="currentColor"
            strokeLinejoin="round"
            strokeWidth="1.7"
          />
          <path
            d="M12 9v4M12 16h.01"
            stroke="currentColor"
            strokeLinecap="round"
            strokeWidth="1.9"
          />
        </>
      );
  }
}
