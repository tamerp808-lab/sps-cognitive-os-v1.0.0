"use client";

import { useEffect, useState } from "react";
import { Brain, Search, Tag } from "lucide-react";
import { api, type MemoryInfo } from "@/lib/api";
import { timeAgo, cn } from "@/lib/utils";

const KIND_COLORS: Record<string, string> = {
  episodic: "badge-accent",
  semantic: "badge-success",
  procedural: "badge-warning",
  conceptual: "badge-muted",
};

export default function MemoryPage() {
  const [memories, setMemories] = useState<MemoryInfo[]>([]);
  const [stats, setStats] = useState<any>(null);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<any[] | null>(null);
  const [loading, setLoading] = useState(true);

  const load = async () => {
    try {
      const s = await api.memoryStats();
      setStats(s);
      setMemories(s.recent_memories ?? []);
    } catch (e) { console.error(e); }
    finally { setLoading(false); }
  };

  useEffect(() => { load(); }, []);

  const search = async () => {
    if (!query.trim()) { setResults(null); return; }
    try {
      const r = await api.memorySearch(query, 50);
      setResults(r.results);
    } catch (e) { console.error(e); }
  };

  const display = results ?? memories.map((m) => ({ ...m, content: null }));

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold flex items-center gap-2">
          <Brain className="w-7 h-7 text-purple-400" />
          Memory
        </h1>
        <p className="text-fg-muted mt-1">Episodic, semantic, procedural, and conceptual memories</p>
      </div>

      {/* Stats */}
      {stats && (
        <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
          <StatBox label="Total" value={stats.total ?? 0} />
          <StatBox label="Links" value={stats.links ?? 0} />
          <StatBox label="Avg strength" value={`${((stats.avg_strength ?? 0) * 100).toFixed(0)}%`} />
          <StatBox label="Kinds" value={Object.keys(stats.by_kind ?? {}).length} />
        </div>
      )}

      {/* Search */}
      <div className="glass-panel p-4">
        <div className="flex items-center gap-2">
          <Search className="w-4 h-4 text-fg-muted" />
          <input
            className="input flex-1"
            placeholder="Search memories by title, tag, or content…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && search()}
          />
          <button className="btn-primary" onClick={search}>Search</button>
        </div>
      </div>

      {/* List */}
      <div className="space-y-2">
        {loading ? (
          <div className="text-center py-12 text-fg-muted">Loading memories…</div>
        ) : display.length === 0 ? (
          <div className="glass-panel p-12 text-center">
            <Brain className="w-12 h-12 mx-auto text-fg-subtle mb-3" />
            <p className="text-fg-muted">No memories yet</p>
            <p className="text-xs text-fg-subtle mt-1">
              Memories are created as the system learns from actions and reflections
            </p>
          </div>
        ) : (
          display.map((m) => (
            <div key={m.id} className="glass-panel p-4 hover:border-accent/30 transition-colors">
              <div className="flex items-start justify-between gap-4">
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 mb-1">
                    <span className={cn("badge", KIND_COLORS[m.kind] ?? "badge-muted")}>{m.kind}</span>
                    <span className="font-medium text-sm truncate">{m.title}</span>
                  </div>
                  {m.content && (
                    <p className="text-xs text-fg-muted mt-1 line-clamp-2">
                      {typeof m.content === "string" ? m.content : JSON.stringify(m.content)}
                    </p>
                  )}
                  {m.tags?.length > 0 && (
                    <div className="flex items-center gap-1 mt-2 flex-wrap">
                      {m.tags.map((t: string) => (
                        <span key={t} className="badge badge-muted">
                          <Tag className="w-2.5 h-2.5" />
                          {t}
                        </span>
                      ))}
                    </div>
                  )}
                </div>
                <div className="text-right flex-shrink-0">
                  <div className="text-xs text-fg-subtle">{timeAgo(m.created_at)}</div>
                  <div className="text-[11px] text-fg-subtle font-mono mt-1">
                    {(m.strength * 100).toFixed(0)}% · {m.access_count}x
                  </div>
                </div>
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}

function StatBox({ label, value }: { label: string; value: any }) {
  return (
    <div className="stat-card">
      <div className="text-xs uppercase tracking-wider text-fg-muted mb-1">{label}</div>
      <div className="text-2xl font-bold">{value}</div>
    </div>
  );
}
