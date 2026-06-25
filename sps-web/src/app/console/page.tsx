"use client";

import { useEffect, useRef, useState } from "react";
import { Terminal, Trash2, Send } from "lucide-react";
import { api } from "@/lib/api";

const PRESET_EVENTS = [
  { type: "system.booted", payload: '{"v":1}' },
  { type: "owner.profile_created", payload: '{"display_name":"Owner","created_at":0}' },
  { type: "memory.created", payload: '{"id":"00000000-0000-0000-0000-000000000001","kind":"semantic","title":"Test memory","content":{"note":"hello"},"tags":[],"origin_tick":0,"created_at":0}' },
];

export default function ConsolePage() {
  const [eventType, setEventType] = useState("");
  const [payload, setPayload] = useState("{}");
  const [history, setHistory] = useState<{ type: string; payload: string; result?: any; error?: string }[]>([]);
  const [sending, setSending] = useState(false);
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => { endRef.current?.scrollIntoView({ behavior: "smooth" }); }, [history]);

  const send = async () => {
    if (!eventType.trim()) return;
    setSending(true);
    let parsed: any;
    try { parsed = JSON.parse(payload); }
    catch { setHistory((h) => [...h, { type: eventType, payload, error: "Invalid JSON" }]); setSending(false); return; }
    try {
      const r = await api.dispatchEvent(eventType, parsed);
      setHistory((h) => [...h, { type: eventType, payload, result: r }]);
    } catch (e: any) {
      setHistory((h) => [...h, { type: eventType, payload, error: e.message }]);
    } finally { setSending(false); }
  };

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold flex items-center gap-2">
          <Terminal className="w-7 h-7 text-accent" />
          Console
        </h1>
        <p className="text-fg-muted mt-1">Dispatch raw events directly to the kernel</p>
      </div>

      {/* Presets */}
      <div className="flex flex-wrap gap-2">
        {PRESET_EVENTS.map((p) => (
          <button key={p.type} className="btn-ghost" onClick={() => { setEventType(p.type); setPayload(p.payload); }}>
            {p.type}
          </button>
        ))}
      </div>

      {/* Input */}
      <div className="glass-panel p-4 space-y-3">
        <div>
          <label className="text-xs uppercase tracking-wider text-fg-muted mb-1.5 block">Event type</label>
          <input className="input font-mono" placeholder="e.g. goal.created" value={eventType} onChange={(e) => setEventType(e.target.value)} />
        </div>
        <div>
          <label className="text-xs uppercase tracking-wider text-fg-muted mb-1.5 block">Payload (JSON)</label>
          <textarea className="input font-mono text-xs min-h-[100px]" value={payload} onChange={(e) => setPayload(e.target.value)} />
        </div>
        <button className="btn-primary w-full" onClick={send} disabled={sending || !eventType.trim()}>
          <Send className="w-4 h-4" />
          Dispatch Event
        </button>
      </div>

      {/* History */}
      <div className="glass-panel p-4">
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-sm font-semibold">History ({history.length})</h2>
          {history.length > 0 && (
            <button className="btn-ghost" onClick={() => setHistory([])}>
              <Trash2 className="w-3.5 h-3.5" />
              Clear
            </button>
          )}
        </div>
        <div className="space-y-2 max-h-[400px] overflow-y-auto">
          {history.length === 0 ? (
            <p className="text-xs text-fg-subtle text-center py-4">No events dispatched yet</p>
          ) : (
            history.slice().reverse().map((h, i) => (
              <div key={i} className="p-3 rounded-lg bg-bg-elevated border border-border">
                <div className="flex items-center gap-2 mb-1">
                  <span className="badge badge-accent font-mono text-xs">{h.type}</span>
                  {h.result && <span className="text-xs text-success">✓ tick {h.result.tick}</span>}
                  {h.error && <span className="text-xs text-danger">✗ {h.error}</span>}
                </div>
                {h.result && (
                  <div className="text-[10px] font-mono text-fg-subtle break-all">
                    hash: {h.result.hash}
                  </div>
                )}
              </div>
            ))
          )}
          <div ref={endRef} />
        </div>
      </div>
    </div>
  );
}
