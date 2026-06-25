const API_BASE = "/api";

export interface HealthResponse {
  status: string;
  kernel_booted: boolean;
  backend: string;
  last_tick: number;
  event_count: number;
  providers: string[];
  timestamp: number;
}

export interface StatsResponse {
  kernel: {
    backend: string;
    last_tick: number;
    event_count: number;
    last_hash: string;
  };
  memory: {
    total: number;
    by_kind: Record<string, number>;
    links: number;
    avg_strength: number;
  };
  goals: {
    total: number;
    active: number;
    total_tasks: number;
    completed_tasks: number;
  };
  providers: string[];
  default_provider: string | null;
}

export interface ProviderInfo {
  id: string;
  name: string;
  kind: string;
  api_url: string;
  model_name: string;
  has_key: boolean;
}

export interface ProvidersResponse {
  providers: ProviderInfo[];
  count: number;
  default_provider: string | null;
}

export interface RegisterProviderRequest {
  kind: string;
  id?: string;
  name?: string;
  api_url: string;
  api_key?: string;
  model_name: string;
}

export interface EventInfo {
  tick: number;
  type: string;
  hash: string;
  prev_hash?: string;
  payload: any;
  wall_time: number;
  actor?: { kind: string; id: string };
}

export interface EventsResponse {
  events: EventInfo[];
  count: number;
}

export interface AgentInfo {
  id: string;
  archetype: string;
  name: string;
  capabilities: {
    can_read_files: boolean;
    can_write_files: boolean;
    can_exec_shell: boolean;
    can_call_llm: boolean;
    can_delegate: boolean;
    can_create_goals: boolean;
  };
  system_prompt: string;
}

export interface MemoryInfo {
  id: string;
  kind: string;
  title: string;
  strength: number;
  access_count: number;
  created_at: number;
  tags: string[];
}

export interface GoalInfo {
  id: string;
  title: string;
  description: string;
  status: string;
  priority: number;
  tasks_total: number;
  objectives_count: number;
  created_at: number;
}

async function fetchJson<T>(url: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${API_BASE}${url}`, {
    headers: { "Content-Type": "application/json", ...(init?.headers || {}) },
    ...init,
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`${res.status}: ${text}`);
  }
  return res.json();
}

export const api = {
  health: () => fetchJson<HealthResponse>("/health"),
  stats: () => fetchJson<StatsResponse>("/stats"),
  state: () => fetchJson<any>("/state"),

  events: (limit = 100, from = 0) =>
    fetchJson<EventsResponse>(`/events?limit=${limit}&from=${from}`),
  dispatchEvent: (eventType: string, payload: any) =>
    fetchJson<{ tick: number; hash: string; type: string }>("/events", {
      method: "POST",
      body: JSON.stringify({ event_type: eventType, payload }),
    }),
  verify: () => fetchJson<any>("/verify"),
  snapshot: () => fetchJson<any>("/snapshot", { method: "POST" }),

  memoryStats: () => fetchJson<any>("/memory"),
  memorySearch: (q: string, limit = 20) =>
    fetchJson<{ results: any[]; count: number }>(`/memory/search?q=${encodeURIComponent(q)}&limit=${limit}`),

  agents: () => fetchJson<{ agents: AgentInfo[]; count: number }>("/agents"),
  dispatchAgent: (archetype: string, title: string, description: string) =>
    fetchJson<any>("/agents/dispatch", {
      method: "POST",
      body: JSON.stringify({ archetype, title, description }),
    }),

  goals: () => fetchJson<{ goals: GoalInfo[]; count: number }>("/goals"),
  verifyGoal: (id: string) => fetchJson<any>(`/goals/${id}/verify`),

  providers: () => fetchJson<ProvidersResponse>("/providers"),
  registerProvider: (req: RegisterProviderRequest) =>
    fetchJson<any>("/providers", { method: "POST", body: JSON.stringify(req) }),
  removeProvider: (id: string) =>
    fetchJson<any>(`/providers/${id}`, { method: "DELETE" }),
  healthcheckProvider: (id: string) =>
    fetchJson<any>(`/providers/${id}/healthcheck`, { method: "POST" }),
  setDefaultProvider: (id: string) =>
    fetchJson<any>("/providers/default", { method: "POST", body: JSON.stringify({ id }) }),

  llmComplete: (req: {
    provider_id?: string;
    user: string;
    system?: string;
    model?: string;
    max_tokens?: number;
    temperature?: number;
  }) => fetchJson<any>("/llm/complete", { method: "POST", body: JSON.stringify(req) }),
};
