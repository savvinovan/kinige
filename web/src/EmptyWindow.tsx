import { useEffect, useState } from "react";
import type { KinigeApi } from "./api/KinigeApi";

/** The slice 0 skeleton: an empty window with the line "Kinige <version>". */
export function EmptyWindow({ api }: { api: KinigeApi }) {
  const [title, setTitle] = useState("");

  useEffect(() => {
    let current = true;
    api
      .appInfo()
      .then((info) => current && setTitle(info.title))
      .catch((error: unknown) => current && setTitle(`Kinige: ${String(error)}`));
    return () => {
      current = false;
    };
  }, [api]);

  return (
    <main className="empty-window">
      <p className="empty-window__title">{title}</p>
    </main>
  );
}
