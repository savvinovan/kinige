import type { ReactNode } from "react";
import type { AttachmentKind } from "../../api/generated/AttachmentKind";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { MediaSourceContext, type MediaSource } from "./mediaUrl";

// Synthetic attachments for stories (and MockApi): gradients with the pixel size written on them, drawn as SVG data URLs. No third-party
// images. The colours are read from the design system tokens when the picture is made, so they follow the palette.

interface DemoPhoto {
  width: number;
  height: number;
  /** A picture with an empty background, like a transparent PNG: the bubble shows through. */
  transparent: boolean;
  /** The file cannot be decoded: the browser fires `error` for the picture. */
  broken: boolean;
}

const photos = new Map<string, DemoPhoto>();
const cache = new Map<string, string>();
const BROKEN = "data:image/png;base64,AAAA";
const THUMB_LONG_SIDE = 640;
const GRADIENTS = [
  ["--color-tile-blue", "--color-accent-default"],
  ["--color-tile-green", "--color-forest-summer-crown-light"],
  ["--color-tile-orange", "--color-forest-autumn-accent"],
] as const;

/** A photo attachment; the picture behind its id is drawn by `demoMediaSource`. */
export function demoPhoto(
  id: string,
  filename: string,
  width: number,
  height: number,
  options: { transparent?: boolean; broken?: boolean; missing?: boolean; byteSize?: number } = {},
): AttachmentView {
  photos.set(id, { width, height, transparent: options.transparent ?? false, broken: options.broken ?? false });
  return {
    id,
    kind: "image",
    filename,
    mime: options.transparent ? "image/png" : "image/jpeg",
    byteSize: options.byteSize ?? width * height,
    width,
    height,
    durationMs: null,
    state: options.missing ? "missing" : "local",
  };
}

/** A file, audio or video attachment. */
export function demoFile(
  id: string,
  filename: string,
  kind: Exclude<AttachmentKind, "image">,
  byteSize: number,
  state: AttachmentView["state"] = "local",
): AttachmentView {
  const mime = kind === "audio" ? "audio/mpeg" : kind === "video" ? "video/mp4" : "application/pdf";
  return { id, kind, filename, mime, byteSize, width: null, height: null, durationMs: null, state };
}

/** The pictures of the stories: a gradient (or an empty background) with the size written on it. */
export const demoMediaSource: MediaSource = (variant, id) => {
  const photo = photos.get(id);
  if (!photo) return BROKEN;
  if (photo.broken) return BROKEN;
  const key = `${variant}/${id}`;
  const cached = cache.get(key);
  if (cached) return cached;
  const url = svgUrl(photo, variant === "thumb", id);
  cache.set(key, url);
  return url;
};

/** Puts the synthetic pictures under `kinige-media` for the subtree. */
export function DemoMedia({ children }: { children: ReactNode }) {
  return <MediaSourceContext.Provider value={demoMediaSource}>{children}</MediaSourceContext.Provider>;
}

function token(name: string): string {
  return typeof document === "undefined" ? "" : getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

function svgUrl({ width, height, transparent }: DemoPhoto, thumb: boolean, id: string): string {
  const scale = thumb ? Math.min(1, THUMB_LONG_SIDE / Math.max(width, height)) : 1;
  const drawnWidth = Math.round(width * scale);
  const drawnHeight = Math.round(height * scale);
  const [from, to] = GRADIENTS[Array.from(id).reduce((sum, symbol) => sum + symbol.charCodeAt(0), 0) % GRADIENTS.length] ?? GRADIENTS[0];
  const text = token("--color-surface-background");
  const fontSize = Math.round(Math.min(drawnWidth / 8, drawnHeight / 4));
  const label = `${width} x ${height}`;
  const background = transparent
    ? ""
    : `<rect width="${drawnWidth}" height="${drawnHeight}" fill="url(#g)"/>`;
  const mark = transparent
    ? `<circle cx="${drawnWidth / 2}" cy="${drawnHeight / 2}" r="${Math.min(drawnWidth, drawnHeight) / 2.4}" fill="url(#g)"/>`
    : "";
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" width="${drawnWidth}" height="${drawnHeight}" viewBox="0 0 ${drawnWidth} ${drawnHeight}">` +
    `<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="${token(from)}"/><stop offset="1" stop-color="${token(to)}"/></linearGradient></defs>` +
    `${background}${mark}` +
    `<text x="50%" y="50%" text-anchor="middle" dominant-baseline="central" font-family="sans-serif" font-size="${fontSize}" font-weight="600" fill="${text}">${label}</text>` +
    `</svg>`;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}
