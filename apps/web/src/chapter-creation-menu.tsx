import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";

export function ChapterCreationMenu({ point, onClose, children }: {
  point: { x: number; y: number }; onClose: () => void; children: ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState(point);
  useLayoutEffect(() => {
    const rect = ref.current?.getBoundingClientRect();
    if (rect) setPosition({ x: Math.max(0, Math.min(point.x, window.innerWidth - rect.width)),
      y: Math.max(0, Math.min(point.y, window.innerHeight - rect.height)) });
  }, [point]);
  useEffect(() => {
    const close = (event: MouseEvent) => { if (!ref.current?.contains(event.target as Node)) onClose(); };
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape") onClose(); };
    document.addEventListener("mousedown", close); document.addEventListener("keydown", escape);
    return () => { document.removeEventListener("mousedown", close); document.removeEventListener("keydown", escape); };
  }, [onClose]);
  return <div ref={ref} className="chapter-creation-menu" role="menu"
    style={{ left: position.x, top: position.y }}>{children}</div>;
}
