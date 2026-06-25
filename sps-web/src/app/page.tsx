"use client";

import { useEffect, useState } from "react";
import {
  Activity,
  Brain,
  Cpu,
  Database,
  Hash,
  Target,
  TrendingUp,
  Zap,
} from "lucide-react";
import { api, type StatsResponse, type HealthResponse } from "@/lib/api";
import { formatHash, formatNumber } from "@/lib/utils";

export default function DashboardPage() {
  const [stats, setStats] = useState<StatsResponse | null>(null);
  const [health, setHealth] = useState<HealthResponse | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const load = async () => {
      try {
        const [s, h] = await Promise.all([api.stats(), api.health()]);
        setStats(s);
        setHealth(h);
      } catch (e) {
        console.error(e);
      } finally {
        setLoading(false);
      }
    };
    load();
    const interval = setInterval(load, 3000);
    return () => clearInterval(interval);
  }, []);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-96">
        <div className="text-fg-muted">Loading kernel state…</div>
      </div>
    );
  }

  const memoryTotal = stats?.memory?.total ?? 0;
  const goalTotal = stats?.goals?.total ?? 0;
  const eventCount = stats?.kernel?.event_count ?? 0;
  const providerCount = stats?.providers?.length ?? 0;

  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-3xl font-bold text-fg">Dashboard</h1>
        <p className="text-fg-muted mt-1">
          Real-time overview of your Cognitive Operating System
        </p>
      </div>

      {/* Stat cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <StatCard
          icon={Database}
          label="Events"
          value={formatNumber(eventCount)}
          sub={`Last tick: ${formatNumber(stats?.kernel?.last_tick ?? 0)}`}
          color="from-blue-500/20 to-cyan-500/10"
        />
        <StatCard
          icon={Brain}
          label="Memories"
          value={formatNumber(memoryTotal)}
          sub={`${stats?.memory?.links ?? 0} links`}
          color="from-purple-500/20 to-pink-500/10"
        />
        <StatCard
          icon={Target}
          label="Goals"
          value={formatNumber(goalTotal)}
          sub={`${stats?.goals?.active ?? 0} active`}
          color="from-emerald-500/20 to-teal-500/10"
        />
        <StatCard
          icon={Zap}
          label="Providers"
          value={formatNumber(providerCount)}
          sub={stats?.default_provider ? `Default: ${stats.default_provider}` : "No default"}
          color="from-amber-500/20 to-orange-500/10"
        />
      </div>

      {/* Two-column layout */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Kernel status */}
        <div className="lg:col-span-2 glass-panel p-6">
          <div className="flex items-center justify-between mb-5">
            <h2 className="text-lg font-semibold flex items-center gap-2">
              <Cpu className="w-5 h-5 text-accent" />
              Kernel Status
            </h2>
            <span className="badge badge-success">
              <span className="w-1.5 h-1.5 rounded-full bg-success animate-pulse" />
              Booted
            </span>
          </div>
          <div className="grid grid-cols-2 gap-4">
            <Field label="Backend" value={stats?.kernel?.backend ?? "—"} mono />
            <Field label="Event Count" value={formatNumber(eventCount)} mono />
            <Field label="Last Tick" value={formatNumber(stats?.kernel?.last_tick ?? 0)} mono />
            <Field
              label="Last Hash"
              value={formatHash(stats?.kernel?.last_hash ?? "", 24)}
              mono
              icon={Hash}
            />
          </div>
        </div>

        {/* Memory breakdown */}
        <div className="glass-panel p-6">
          <h2 className="text-lg font-semibold flex items-center gap-2 mb-5">
            <Brain className="w-5 h-5 text-purple-400" />
            Memory
          </h2>
          {memoryTotal === 0 ? (
            <p className="text-sm text-fg-subtle">No memories yet.</p>
          ) : (
            <div className="space-y-3">
              {Object.entries(stats?.memory?.by_kind ?? {}).map(([kind, count]) => (
                <div key={kind} className="flex items-center justify-between">
                  <span className="text-sm capitalize text-fg-muted">{kind}</span>
                  <div className="flex items-center gap-2">
                    <div className="w-24 h-1.5 bg-bg-elevated rounded-full overflow-hidden">
                      <div
                        className="h-full bg-gradient-to-r from-purple-500 to-pink-500"
                        style={{ width: `${(count / memoryTotal) * 100}%` }}
                      />
                    </div>
                    <span className="text-sm font-mono w-8 text-right">{count}</span>
                  </div>
                </div>
              ))}
              <div className="pt-3 border-t border-border-subtle mt-3">
                <div className="flex justify-between text-sm">
                  <span className="text-fg-muted">Avg strength</span>
                  <span className="font-mono">
                    {((stats?.memory?.avg_strength ?? 0) * 100).toFixed(0)}%
                  </span>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Goal progress */}
      <div className="glass-panel p-6">
        <h2 className="text-lg font-semibold flex items-center gap-2 mb-5">
          <Target className="w-5 h-5 text-emerald-400" />
          Goal Progress
        </h2>
        {goalTotal === 0 ? (
          <p className="text-sm text-fg-subtle">No goals yet.</p>
        ) : (
          <div className="space-y-4">
            <div className="flex items-center justify-between text-sm">
              <span className="text-fg-muted">Total tasks</span>
              <span className="font-mono">{stats?.goals?.total_tasks ?? 0}</span>
            </div>
            <div className="flex items-center justify-between text-sm">
              <span className="text-fg-muted">Completed</span>
              <span className="font-mono text-success">
                {stats?.goals?.completed_tasks ?? 0}
              </span>
            </div>
            <div className="w-full h-2 bg-bg-elevated rounded-full overflow-hidden">
              <div
                className="h-full bg-gradient-to-r from-emerald-500 to-teal-400 transition-all"
                style={{
                  width: `${
                    stats?.goals?.total_tasks
                      ? ((stats?.goals?.completed_tasks ?? 0) / stats.goals.total_tasks) * 100
                      : 0
                  }%`,
                }}
              />
            </div>
          </div>
        )}
      </div>

      {/* Provider status */}
      <div className="glass-panel p-6">
        <h2 className="text-lg font-semibold flex items-center gap-2 mb-5">
          <Zap className="w-5 h-5 text-amber-400" />
          AI Providers
        </h2>
        {providerCount === 0 ? (
          <div className="text-center py-8">
            <Zap className="w-12 h-12 mx-auto text-fg-subtle mb-3" />
            <p className="text-sm text-fg-muted mb-1">No providers configured</p>
            <p className="text-xs text-fg-subtle">
              Add a cloud model to enable LLM-powered features
            </p>
          </div>
        ) : (
          <div className="space-y-2">
            {stats?.providers?.map((p) => (
              <div
                key={p}
                className="flex items-center justify-between p-3 rounded-lg bg-bg-elevated"
              >
                <div className="flex items-center gap-3">
                  <div className="w-8 h-8 rounded-lg bg-accent/20 flex items-center justify-center">
                    <Zap className="w-4 h-4 text-accent" />
                  </div>
                  <div>
                    <div className="text-sm font-medium">{p}</div>
                    <div className="text-xs text-fg-subtle">Provider ID</div>
                  </div>
                </div>
                {stats?.default_provider === p && (
                  <span className="badge badge-accent">Default</span>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function StatCard({
  icon: Icon,
  label,
  value,
  sub,
  color,
}: {
  icon: any;
  label: string;
  value: string;
  sub: string;
  color: string;
}) {
  return (
    <div className="stat-card relative overflow-hidden">
      <div className={`absolute inset-0 bg-gradient-to-br ${color} opacity-50`} />
      <div className="relative">
        <div className="flex items-center justify-between mb-3">
          <span className="text-xs uppercase tracking-wider text-fg-muted font-medium">
            {label}
          </span>
          <Icon className="w-4 h-4 text-fg-muted" />
        </div>
        <div className="text-3xl font-bold text-fg">{value}</div>
        <div className="text-xs text-fg-subtle mt-1">{sub}</div>
      </div>
    </div>
  );
}

function Field({
  label,
  value,
  mono,
  icon: Icon,
}: {
  label: string;
  value: string;
  mono?: boolean;
  icon?: any;
}) {
  return (
    <div>
      <div className="text-xs text-fg-subtle uppercase tracking-wider mb-1">{label}</div>
      <div className={`text-sm text-fg ${mono ? "font-mono" : ""} flex items-center gap-1.5`}>
        {Icon && <Icon className="w-3.5 h-3.5 text-fg-muted" />}
        {value}
      </div>
    </div>
  );
}
