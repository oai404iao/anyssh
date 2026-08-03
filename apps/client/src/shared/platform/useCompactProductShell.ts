import { useEffect, useState } from "react";
import { isAndroidPlatform } from "./runtime";

const COMPACT_PRODUCT_SHELL_QUERY = "(max-width: 780px)";

export function useCompactProductShell(): boolean {
  const android = isAndroidPlatform();
  const [compact, setCompact] = useState(() => {
    if (android) return true;
    return (
      typeof window !== "undefined" &&
      window.matchMedia(COMPACT_PRODUCT_SHELL_QUERY).matches
    );
  });

  useEffect(() => {
    if (android) return;
    const media = window.matchMedia(COMPACT_PRODUCT_SHELL_QUERY);
    const update = () => setCompact(media.matches);
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, [android]);

  return compact;
}
