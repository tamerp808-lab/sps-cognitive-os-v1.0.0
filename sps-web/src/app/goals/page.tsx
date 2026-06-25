"use client";

import { useEffect, useState } from "react";
import { Target, CheckCircle2, Circle, Clock, XCircle } from "lucide-react";
import { api, type GoalInfo } from "@/lib/api";
import { cn, timeAgo } from "@/lib/utils";

const STATUS_ICONS: Record<string, any> = {
  pending: Clock,
  planning: Clock,
  active: Circle,
  blocked: XCircle,
  completed: CheckCircle2,
  abandoned: XCircle,
};

const STATUS_COLORS: Record<string, string> = {
  pending: "text-fg-subtle",
  planning: "text-warning",
  active: "text-accent",
  blocked: "text-danger",
  completed: "text-success",
  abandoned: "text-fg-subtle",
};

export default function GoalsPage() {
  const [goals, setGoals] = useState<GoalInfo[]>([]);
  const [loading, setLoading] = useState(true);

  const load = async () => {
    try { const r = await api.goals(); setGoals(r.goals); }
    catch (e) { console.error(e); }
    finally { setLoading(false); }
  };

  useEffect(() => { load(); const i = setInterval(load, 5000); return () => clearInterval(i); }, []);

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold flex items-center gap-2">
          <Target className="w-7 h-7 text-emerald-400" />
          Goals
        </h1>
        <p className="text-fg-muted mt-1">Goal → Objective → Milestone → Task hierarchy</p>
      </div>

      {loading ? (
        <div className="text-center py-12 text-fg-muted">Loading goals…</div>
      ) : goals.length === 0 ? (
        <div className="glass-panel p-12 text-center">
          <Target className="w-12 h-12 mx-auto text-fg-subtle mb-3" />
          <p className="text-fg-muted">No goals yet</p>
        </div>
      ) : (
        <div className="space-y-3">
          {goals.map((g) => {
            const Icon = STATUS_ICONS[g.status] ?? Circle;
            return (
              <div key={g.id} className="glass-panel p-5">
                <div className="flex items-start justify-between gap-4">
                  <div className="flex-1">
                    <div className="flex items-center gap-2 mb-1">
                      <Icon className={cn("w-4 h-4", STATUS_COLORS[g.status])} />
                      <span className="font-semibold">{g.title}</span>
                      <span className={cn("badge", `badge-${g.status === "completed" ? "success" : g.status === "active" ? "accent" : g.status === "blocked" ? "danger" : "muted"}`)}>
                        {g.status}
                      </span>
                      {g.priority > 5 && <span className="badge badge-warning">P{g.priority}</span>}
                    </div>
                    {g.description && (
                      <p className="text-sm text-fg-muted mt-1 line-clamp-2">{g.description}</p>
                    )}
                    <div className="flex items-center gap-4 mt-3 text-xs text-fg-subtle">
                      <span className="font-mono">{g.id.slice(0, 8)}…</span>
                      <span>{g.objectives_count} objectives</span>
                      <span>{g.tasks_total} tasks</span>
                      <span>{timeAgo(g.created_at)}</span>
                    </div>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
