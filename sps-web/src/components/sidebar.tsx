"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import {
  Activity,
  Brain,
  Cpu,
  Database,
  Layers,
  Settings,
  Sparkles,
  Target,
  Terminal,
  Zap,
} from "lucide-react";
import { cn } from "@/lib/utils";

const nav = [
  { href: "/", label: "Dashboard", icon: Activity },
  { href: "/providers", label: "AI Providers", icon: Zap },
  { href: "/chat", label: "Chat", icon: Sparkles },
  { href: "/memory", label: "Memory", icon: Brain },
  { href: "/agents", label: "Agents", icon: Cpu },
  { href: "/goals", label: "Goals", icon: Target },
  { href: "/events", label: "Events", icon: Layers },
  { href: "/console", label: "Console", icon: Terminal },
  { href: "/settings", label: "Settings", icon: Settings },
];

export function Sidebar() {
  const pathname = usePathname();

  return (
    <aside className="fixed left-0 top-0 bottom-0 w-64 bg-bg-surface border-r border-border flex flex-col">
      <div className="p-6 border-b border-border">
        <div className="flex items-center gap-3">
          <div className="w-9 h-9 rounded-xl bg-gradient-to-br from-accent to-purple-600 flex items-center justify-center">
            <Database className="w-5 h-5 text-white" />
          </div>
          <div>
            <div className="font-bold text-fg text-lg leading-none">SPS</div>
            <div className="text-[11px] text-fg-subtle mt-1 leading-none">
              Cognitive OS
            </div>
          </div>
        </div>
      </div>

      <nav className="flex-1 p-3 space-y-0.5 overflow-y-auto">
        {nav.map((item) => {
          const Icon = item.icon;
          const active = pathname === item.href;
          return (
            <Link
              key={item.href}
              href={item.href}
              className={cn("nav-item", active && "active")}
            >
              <Icon className="w-4 h-4" />
              {item.label}
            </Link>
          );
        })}
      </nav>

      <div className="p-4 border-t border-border">
        <div className="text-[11px] text-fg-subtle uppercase tracking-wider mb-2">
          Kernel Status
        </div>
        <div className="flex items-center gap-2">
          <span className="w-2 h-2 rounded-full bg-success animate-pulse" />
          <span className="text-xs text-fg-muted">Online</span>
        </div>
        <div className="text-[10px] text-fg-subtle mt-1 font-mono">
          v1.0.0 · local-first
        </div>
      </div>
    </aside>
  );
}
