"use client";

import { useEffect, useState } from "react";
import {
  Cloud,
  Globe,
  Hash,
  Key,
  Plus,
  Server,
  Trash2,
  Zap,
  Check,
} from "lucide-react";
import { api, type ProviderInfo } from "@/lib/api";
import { cn } from "@/lib/utils";

const PROVIDER_PRESETS = [
  { kind: "openai", name: "OpenAI", apiUrl: "https://api.openai.com/v1", models: ["gpt-4o", "gpt-4o-mini", "gpt-4-turbo", "o1-preview"], requiresKey: true, color: "from-emerald-500 to-teal-500" },
  { kind: "anthropic", name: "Anthropic", apiUrl: "https://api.anthropic.com", models: ["claude-3-5-sonnet-20241022", "claude-3-5-haiku-20241022", "claude-3-opus-20240229"], requiresKey: true, color: "from-amber-500 to-orange-500" },
  { kind: "openrouter", name: "OpenRouter", apiUrl: "https://openrouter.ai/api/v1", models: ["anthropic/claude-3.5-sonnet", "openai/gpt-4o", "google/gemini-pro-1.5", "meta-llama/llama-3.1-70b-instruct"], requiresKey: true, color: "from-blue-500 to-indigo-500" },
  { kind: "groq", name: "Groq", apiUrl: "https://api.groq.com/openai/v1", models: ["llama-3.3-70b-versatile", "llama-3.1-8b-instant", "mixtral-8x7b-32768"], requiresKey: true, color: "from-orange-500 to-red-500" },
  { kind: "deepseek", name: "DeepSeek", apiUrl: "https://api.deepseek.com/v1", models: ["deepseek-chat", "deepseek-reasoner"], requiresKey: true, color: "from-violet-500 to-purple-500" },
  { kind: "ollama", name: "Ollama (Local)", apiUrl: "http://localhost:11434", models: ["llama3.2", "qwen2.5", "mistral", "phi3"], requiresKey: false, color: "from-slate-500 to-zinc-500" },
  { kind: "lmstudio", name: "LM Studio (Local)", apiUrl: "http://localhost:1234/v1", models: ["local-model"], requiresKey: false, color: "from-cyan-500 to-blue-500" },
  { kind: "custom", name: "Custom Endpoint", apiUrl: "", models: [], requiresKey: false, color: "from-fuchsia-500 to-pink-500" },
];

export default function ProvidersPage() {
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [defaultProvider, setDefaultProvider] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [showForm, setShowForm] = useState(false);
  const [selectedPreset, setSelectedPreset] = useState(PROVIDER_PRESETS[0]);
  const [form, setForm] = useState({ id: "", name: "", api_url: PROVIDER_PRESETS[0].apiUrl, api_key: "", model_name: PROVIDER_PRESETS[0].models[0] ?? "" });
  const [registering, setRegistering] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [healthchecks, setHealthchecks] = useState<Record<string, any>>({});

  const load = async () => {
    try {
      const res = await api.providers();
      setProviders(res.providers);
      setDefaultProvider(res.default_provider);
    } catch (e) { console.error(e); }
    finally { setLoading(false); }
  };

  useEffect(() => { load(); }, []);

  const selectPreset = (preset: typeof PROVIDER_PRESETS[0]) => {
    setSelectedPreset(preset);
    setForm({ id: "", name: "", api_url: preset.apiUrl, api_key: "", model_name: preset.models[0] ?? "" });
    setError(null);
  };

  const handleRegister = async () => {
    setRegistering(true); setError(null);
    try {
      await api.registerProvider({ kind: selectedPreset.kind, id: form.id || undefined, name: form.name || undefined, api_url: form.api_url, api_key: form.api_key || undefined, model_name: form.model_name });
      setShowForm(false);
      setForm({ id: "", name: "", api_url: selectedPreset.apiUrl, api_key: "", model_name: selectedPreset.models[0] ?? "" });
      await load();
    } catch (e: any) { setError(e.message); }
    finally { setRegistering(false); }
  };

  const handleRemove = async (id: string) => { if (!confirm(`Remove provider "${id}"?`)) return; await api.removeProvider(id); await load(); };
  const handleHealthcheck = async (id: string) => { try { const res = await api.healthcheckProvider(id); setHealthchecks((p) => ({ ...p, [id]: res })); } catch (e: any) { setHealthchecks((p) => ({ ...p, [id]: { healthy: false, error: e.message } })); } };
  const handleSetDefault = async (id: string) => { await api.setDefaultProvider(id); await load(); };

  return (
    <div className="space-y-8">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold">AI Providers</h1>
          <p className="text-fg-muted mt-1">Connect cloud and local LLM models to your kernel</p>
        </div>
        <button className="btn-primary" onClick={() => setShowForm(!showForm)}>
          <Plus className="w-4 h-4" />
          Add Provider
        </button>
      </div>

      {showForm && (
        <div className="glass-panel p-6 space-y-6">
          <div>
            <h2 className="text-lg font-semibold mb-1">Choose a provider</h2>
            <p className="text-sm text-fg-muted">Select a preset or configure a custom endpoint</p>
          </div>
          <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
            {PROVIDER_PRESETS.map((preset) => (
              <button key={preset.kind} onClick={() => selectPreset(preset)}
                className={cn("p-4 rounded-xl border text-left transition-all",
                  selectedPreset.kind === preset.kind ? "border-accent bg-accent/10" : "border-border bg-bg-elevated hover:bg-bg-hover")}>
                <div className={cn("w-8 h-8 rounded-lg bg-gradient-to-br mb-2", preset.color)} />
                <div className="text-sm font-medium">{preset.name}</div>
                <div className="text-[11px] text-fg-subtle uppercase tracking-wide mt-1">{preset.requiresKey ? "API key" : "Local"}</div>
              </button>
            ))}
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4 pt-4 border-t border-border">
            <div>
              <label className="text-xs uppercase tracking-wider text-fg-muted mb-1.5 block">Provider ID (optional)</label>
              <input className="input" placeholder={`defaults to "${selectedPreset.kind}"`} value={form.id} onChange={(e) => setForm({ ...form, id: e.target.value })} />
            </div>
            <div>
              <label className="text-xs uppercase tracking-wider text-fg-muted mb-1.5 block">Display name (optional)</label>
              <input className="input" placeholder={selectedPreset.name} value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} />
            </div>
            <div className="md:col-span-2">
              <label className="text-xs uppercase tracking-wider text-fg-muted mb-1.5 block flex items-center gap-1.5"><Globe className="w-3.5 h-3.5" /> API URL</label>
              <input className="input font-mono" placeholder="https://api.example.com/v1" value={form.api_url} onChange={(e) => setForm({ ...form, api_url: e.target.value })} />
            </div>
            {selectedPreset.requiresKey && (
              <div className="md:col-span-2">
                <label className="text-xs uppercase tracking-wider text-fg-muted mb-1.5 block flex items-center gap-1.5"><Key className="w-3.5 h-3.5" /> API Key</label>
                <input type="password" className="input font-mono" placeholder="sk-..." value={form.api_key} onChange={(e) => setForm({ ...form, api_key: e.target.value })} />
                <p className="text-xs text-fg-subtle mt-1.5">Stored locally. Never sent to any server except the provider.</p>
              </div>
            )}
            <div>
              <label className="text-xs uppercase tracking-wider text-fg-muted mb-1.5 block">Model name</label>
              {selectedPreset.models.length > 0 ? (
                <select className="input" value={form.model_name} onChange={(e) => setForm({ ...form, model_name: e.target.value })}>
                  {selectedPreset.models.map((m) => (<option key={m} value={m}>{m}</option>))}
                </select>
              ) : (
                <input className="input font-mono" placeholder="model-name" value={form.model_name} onChange={(e) => setForm({ ...form, model_name: e.target.value })} />
              )}
            </div>
          </div>
          {error && (<div className="p-3 rounded-lg bg-danger/10 border border-danger/30 text-sm text-danger">{error}</div>)}
          <div className="flex justify-end gap-2 pt-4 border-t border-border">
            <button className="btn-secondary" onClick={() => setShowForm(false)}>Cancel</button>
            <button className="btn-primary" onClick={handleRegister} disabled={registering || !form.api_url || !form.model_name}>
              {registering ? "Registering…" : "Register Provider"}
            </button>
          </div>
        </div>
      )}

      {loading ? (
        <div className="text-center py-12 text-fg-muted">Loading providers…</div>
      ) : providers.length === 0 && !showForm ? (
        <div className="glass-panel p-12 text-center">
          <Cloud className="w-16 h-16 mx-auto text-fg-subtle mb-4" />
          <h3 className="text-lg font-medium mb-2">No providers configured</h3>
          <p className="text-sm text-fg-muted mb-4 max-w-md mx-auto">
            Add a cloud provider like OpenAI, Anthropic, or OpenRouter to enable LLM-powered reasoning, reflection, and code generation.
          </p>
          <button className="btn-primary" onClick={() => setShowForm(true)}>
            <Plus className="w-4 h-4" /> Add your first provider
          </button>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {providers.map((p) => {
            const preset = PROVIDER_PRESETS.find((pr) => pr.kind === p.kind) ?? PROVIDER_PRESETS[7];
            const hc = healthchecks[p.id];
            const isDefault = defaultProvider === p.id;
            return (
              <div key={p.id} className="glass-panel p-5">
                <div className="flex items-start justify-between mb-4">
                  <div className="flex items-center gap-3">
                    <div className={cn("w-10 h-10 rounded-xl bg-gradient-to-br flex items-center justify-center", preset.color)}>
                      <Zap className="w-5 h-5 text-white" />
                    </div>
                    <div>
                      <div className="font-semibold flex items-center gap-2">
                        {p.name || p.id}
                        {isDefault && <span className="badge badge-accent">Default</span>}
                      </div>
                      <div className="text-xs text-fg-subtle font-mono">{p.kind}</div>
                    </div>
                  </div>
                  <button className="btn-ghost text-danger hover:text-danger" onClick={() => handleRemove(p.id)}>
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>
                <div className="space-y-2 mb-4">
                  <div className="flex items-center gap-2 text-sm">
                    <Globe className="w-3.5 h-3.5 text-fg-subtle" />
                    <span className="text-fg-muted font-mono text-xs truncate">{p.api_url || "—"}</span>
                  </div>
                  <div className="flex items-center gap-2 text-sm">
                    <Hash className="w-3.5 h-3.5 text-fg-subtle" />
                    <span className="text-fg-muted font-mono text-xs truncate">{p.model_name || "—"}</span>
                  </div>
                  <div className="flex items-center gap-2 text-sm">
                    <Key className="w-3.5 h-3.5 text-fg-subtle" />
                    <span className="text-fg-muted text-xs">{p.has_key ? "API key set" : "No key required"}</span>
                  </div>
                </div>
                <div className="flex items-center justify-between pt-4 border-t border-border">
                  {hc ? (
                    <span className={cn("badge", hc.healthy ? "badge-success" : "badge-danger")}>
                      <span className={cn("w-1.5 h-1.5 rounded-full", hc.healthy ? "bg-success" : "bg-danger")} />
                      {hc.healthy ? `Healthy · ${hc.latency_ms}ms` : "Unhealthy"}
                    </span>
                  ) : (
                    <span className="text-xs text-fg-subtle">Not checked yet</span>
                  )}
                  <button className="btn-ghost" onClick={() => handleHealthcheck(p.id)}>
                    <Server className="w-3.5 h-3.5" /> Healthcheck
                  </button>
                </div>
                {!isDefault && (
                  <button className="w-full mt-3 btn-secondary text-xs" onClick={() => handleSetDefault(p.id)}>
                    <Check className="w-3.5 h-3.5" /> Set as default
                  </button>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
