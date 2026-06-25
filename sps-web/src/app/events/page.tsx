"use client";

import { useEffect, useState } from "react";
import { Layers, Hash, Clock } from "lucide-react";
import { api, type EventInfo } from "@/lib/api";
import { cn, formatHash, formatTime } from "@/lib/utils";

export default function EventsPage() {
  const [events, setEvents] = useState<EventInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState("");

  const load = async () => {
    try { const r = await api.events(200); setEvents(r.events.reverse()); }
    catch (e) { console.error(e); }
    finally { setLoading(false); }
  };

  useEffect(() => { load(); const i = setInterval(load, 2000); return () => clearInterval(i); }, []);

  const filtered = filter ? events.filter((e) => e.type.includes(filter)) : events;

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold flex items-center gap-2">
          <Layers className="w-7 h-7 text-blue-400" />
          Event Stream
        </h1>
        <p className="text-fg-muted mt-1">Hash-chained, immutable event log — the source of truth</p>
      </div>

      <div className="glass-panel p-3 flex items-center gap-2">
        <input
          className="input flex-1"
          placeholder="Filter by event type (e.g. goal, memory, effect)…"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
        <span className="text-xs text-fg-subtle px-2">{filtered.length} events</span>
      </div>

      <div className="glass-panel overflow-hidden">
        {loading ? (
          <div className="p-12 text-center text-fg-muted">Loading events…</div>
        ) : filtered.length === 0 ? (
          <div className="p-12 text-center text-fg-muted">No events match filter</div>
        ) : (
          <div className="divide-y divide-border-subtle max-h-[600px] overflow-y-auto">
            {filtered.map((e) => (
              <div key={e.tick} className="p-4 hover:bg-bg-hover transition-colors">
                <div className="flex items-center gap-3 mb-1.5">
                  <span className="text-xs font-mono text-fg-subtle w-12">#{e.tick}</span>
                  <span className="badge badge-accent font-mono text-xs">{e.type}</span>
                  <span className="flex items-center gap-1 text-xs text-fg-subtle ml-auto">
                    <Clock className="w-3 h-3" />
                    {formatTime(e.wall_time)}
                  </span>
                </div>
                <div className="flex items-center gap-1.5 text-[11px] font-mono text-fg-subtle ml-[60px]">
                  <Hash className="w-3 h-3" />
                  {formatHash(e.hash, 32)}
                </div>
                {e.payload && Object.keys(e.payload).length > 0 && (
                  <details className="ml-[60px] mt-1.5">
                    <summary className="text-xs text-fg-muted cursor-pointer hover:text-fg">Payload</summary>
                    <pre className="text-[10px] font-mono text-fg-subtle mt-1 p-2 bg-bg-base rounded overflow-x-auto">
                      {JSON.stringify(e.payload, null, 2)}
                    </pre>
                  </details>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
