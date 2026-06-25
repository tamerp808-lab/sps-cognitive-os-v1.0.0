"use client";

import { useEffect, useState } from "react";
import { Cpu, Send, Activity } from "lucide-react";
import { api, type AgentInfo } from "@/lib/api";
import { cn } from "@/lib/utils";

const ARCHETYPE_COLORS: Record<string, string> = {
  architect: "from-blue-500 to-indigo-500",
  developer: "from-emerald-500 to-teal-500",
  reviewer: "from-amber-500 to-orange-500",
  tester: "from-pink-500 to-rose-500",
  devops: "from-violet-500 to-purple-500",
  researcher: "from-cyan-500 to-blue-500",
};

const ARCHETYPE_ICONS: Record<string, string> = {
  architect: "🏛",
  developer: "💻",
  reviewer: "🔍",
  tester: "🧪",
  devops: "🚀",
  researcher: "📚",
};

export default function AgentsPage() {
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [dispatch, setDispatch] = useState({ archetype: "developer", title: "", description: "" });
  const [result, setResult] = useState<any>(null);
  const [dispatching, setDispatching] = useState(false);

  useEffect(() => {
    api.agents().then((r) => setAgents(r.agents)).catch(console.error).finally(() => setLoading(false));
  }, []);

  const handleDispatch = async () => {
    if (!dispatch.title.trim()) return;
    setDispatching(true);
    try {
      const r = await api.dispatchAgent(dispatch.archetype, dispatch.title, dispatch.description);
      setResult(r);
    } catch (e: any) { setResult({ error: e.message }); }
    finally { setDispatching(false); }
  };

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold flex items-center gap-2">
          <Cpu className="w-7 h-7 text-accent" />
          Agents
        </h1>
        <p className="text-fg-muted mt-1">Six built-in agent archetypes for specialized work</p>
      </div>

      {/* Agent grid */}
      {loading ? (
        <div className="text-center py-12 text-fg-muted">Loading agents…</div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {agents.map((a) => (
            <div key={a.id} className="glass-panel p-5">
              <div className="flex items-center gap-3 mb-3">
                <div className={cn("w-10 h-10 rounded-xl bg-gradient-to-br flex items-center justify-center text-lg", ARCHETYPE_COLORS[a.archetype])}>
                  {ARCHETYPE_ICONS[a.archetype]}
                </div>
                <div>
                  <div className="font-semibold">{a.name}</div>
                  <div className="text-xs text-fg-subtle capitalize">{a.archetype}</div>
                </div>
              </div>
              <p className="text-xs text-fg-muted line-clamp-3 mb-3">{a.system_prompt}</p>
              <div className="flex flex-wrap gap-1">
                {a.capabilities.can_read_files && <span className="badge badge-muted">read</span>}
                {a.capabilities.can_write_files && <span className="badge badge-muted">write</span>}
                {a.capabilities.can_exec_shell && <span className="badge badge-muted">shell</span>}
                {a.capabilities.can_call_llm && <span className="badge badge-accent">llm</span>}
                {a.capabilities.can_delegate && <span className="badge badge-muted">delegate</span>}
                {a.capabilities.can_create_goals && <span className="badge badge-muted">goals</span>}
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Dispatch */}
      <div className="glass-panel p-6">
        <h2 className="text-lg font-semibold flex items-center gap-2 mb-4">
          <Send className="w-5 h-5 text-accent" />
          Dispatch Task
        </h2>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
          <select className="input" value={dispatch.archetype} onChange={(e) => setDispatch({ ...dispatch, archetype: e.target.value })}>
            {["architect", "developer", "reviewer", "tester", "devops", "researcher"].map((a) => (
              <option key={a} value={a}>{a}</option>
            ))}
          </select>
          <input className="input" placeholder="Task title" value={dispatch.title} onChange={(e) => setDispatch({ ...dispatch, title: e.target.value })} />
          <button className="btn-primary" onClick={handleDispatch} disabled={dispatching || !dispatch.title}>
            {dispatching ? "Dispatching…" : "Dispatch"}
          </button>
        </div>
        <input className="input mt-3" placeholder="Description (optional)" value={dispatch.description} onChange={(e) => setDispatch({ ...dispatch, description: e.target.value })} />

        {result && (
          <div className="mt-4 p-4 rounded-lg bg-bg-elevated border border-border">
            {result.error ? (
              <p className="text-sm text-danger">{result.error}</p>
            ) : (
              <div className="space-y-2 text-sm">
                <div className="flex items-center gap-2 text-success">
                  <Activity className="w-4 h-4" />
                  Dispatched successfully
                </div>
                <div className="text-xs text-fg-muted font-mono">
                  Agent: {result.agent_id}
                </div>
                <div className="text-xs text-fg-muted font-mono">
                  Task: {result.task_id}
                </div>
                {result.messages?.length > 0 && (
                  <div className="text-xs text-fg-muted">{result.messages.length} message(s) generated</div>
                )}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
