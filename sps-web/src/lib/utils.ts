import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function formatNumber(n: number): string {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + "M";
  if (n >= 1_000) return (n / 1_000).toFixed(1) + "K";
  return n.toString();
}

export function formatHash(hash: string, len = 16): string {
  if (!hash) return "—";
  return hash.length > len ? hash.slice(0, len) + "…" : hash;
}

export function formatTime(ms: number): string {
  if (!ms) return "—";
  const d = new Date(ms);
  return d.toLocaleTimeString("en-US", { hour12: false }) + " " + d.toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

export function timeAgo(ms: number): string {
  if (!ms) return "never";
  const now = Date.now();
  const diff = now - ms;
  if (diff < 60_000) return Math.floor(diff / 1000) + "s ago";
  if (diff < 3_600_000) return Math.floor(diff / 60_000) + "m ago";
  if (diff < 86_400_000) return Math.floor(diff / 3_600_000) + "h ago";
  return Math.floor(diff / 86_400_000) + "d ago";
}
