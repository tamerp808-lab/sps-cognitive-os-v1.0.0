"use client";

import { useEffect, useState } from "react";
import { Settings, ShieldCheck, Database, Cpu, Activity } from "lucide-react";
import { api } from "@/lib/api";

export default function SettingsPage() {
  const [health, setHealth] = useState<any>(null);
  const [verify, setVerify] = useState<any>(null);

  useEffect(() => {
    api.health().then(setHealth).catch(console.error);
  }, []);

  const runVerify = async () => {
    try { setVerify(await api.verify()); }
    catch (e: any) { setVerify({ error: e.message }); }
  };

  const takeSnapshot = async () => {
    try { await api.snapshot(); alert("Snapshot taken"); }
    catch (e: any) { alert("Snapshot failed: " + e.message); }
  };

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold flex items-center gap-2">
          <Settings className="w-7 h-7 text-fg-muted" />
          Settings
        </h1>
        <p className="text-fg-muted mt-1">System information and kernel operations</p>
      </div>

      {/* System info */}
      <div className="glass-panel p-6">
        <h2 className="text-lg font-semibold flex items-center gap-2 mb-4">
          <Cpu className="w-5 h-5 text-accent" />
          System
        </h2>
        {health && (
          <div className="grid grid-cols-2 gap-4">
            <Field label="Status" value={health.status} />
            <Field label="Backend" value={health.backend} mono />
            <Field label="Kernel booted" value={health.kernel_booted ? "Yes" : "No"} />
            <Field label="Event count" value={health.event_count} mono />
            <Field label="Last tick" value={health.last_tick} mono />
            <Field label="Providers" value={health.providers?.length ?? 0} mono />
          </div>
        )}
      </div>

      {/* Operations */}
      <div className="glass-panel p-6">
        <h2 className="text-lg font-semibold flex items-center gap-2 mb-4">
          <Activity className="w-5 h-5 text-accent" />
          Kernel Operations
        </h2>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          <button className="btn-secondary justify-start" onClick={runVerify}>
            <ShieldCheck className="w-4 h-4" />
            Verify hash chain
          </button>
          <button className="btn-secondary justify-start" onClick={takeSnapshot}>
            <Database className="w-4 h-4" />
            Take snapshot
          </button>
        </div>
        {verify && (
          <div className="mt-4 p-3 rounded-lg bg-bg-elevated border border-border text-sm">
            {verify.error ? (
              <span className="text-danger">{verify.error}</span>
            ) : (
              <div className="space-y-1">
                <div className={verify.failure ? "text-danger" : "text-success"}>
                  {verify.failure ? "✗ Verification failed" : "✓ Chain intact"}
                </div>
                <div className="text-xs text-fg-muted font-mono">
                  {verify.events_verified} events verified · {verify.elapsed_us}μs
                </div>
                {verify.failure && (
                  <div className="text-xs text-danger">{verify.failure_detail}</div>
                )}
              </div>
            )}
          </div>
        )}
      </div>

      {/* About */}
      <div className="glass-panel p-6">
        <h2 className="text-lg font-semibold mb-4">About SPS</h2>
        <p className="text-sm text-fg-muted mb-3">
          SPS is a Cognitive Operating System built on a deterministic, event-sourced kernel.
          All state changes are hash-chained events; the entire system can be replayed from
          the event log.
        </p>
        <div className="text-xs text-fg-subtle space-y-1">
          <div>Version: 1.0.0</div>
          <div>License: MIT OR Apache-2.0</div>
          <div>Architecture: Local-first, single-owner</div>
          <div>Kernel language: Rust</div>
          <div>UI: Next.js + React + Tailwind</div>
        </div>
      </div>
    </div>
  );
}

function Field({ label, value, mono }: { label: string; value: any; mono?: boolean }) {
  return (
    <div>
      <div className="text-xs uppercase tracking-wider text-fg-muted mb-1">{label}</div>
      <div className={`text-sm ${mono ? "font-mono" : ""}`}>{String(value)}</div>
    </div>
  );
}
