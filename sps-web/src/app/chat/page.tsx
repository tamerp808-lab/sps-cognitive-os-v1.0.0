"use client";

import { useEffect, useRef, useState } from "react";
import { Send, Sparkles, User, Cpu, AlertCircle } from "lucide-react";
import { api } from "@/lib/api";
import { cn } from "@/lib/utils";

interface Message {
  role: "user" | "assistant" | "system";
  content: string;
  provider?: string;
  model?: string;
  elapsedMs?: number;
  error?: string;
}

export default function ChatPage() {
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const [defaultProvider, setDefaultProvider] = useState<string | null>(null);
  const [systemPrompt, setSystemPrompt] = useState("You are a helpful AI assistant integrated into the SPS Cognitive Operating System. Be concise and accurate.");
  const [showSystem, setShowSystem] = useState(false);
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    api.providers().then((r) => setDefaultProvider(r.default_provider)).catch(() => {});
  }, []);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const send = async () => {
    if (!input.trim() || sending) return;
    if (!defaultProvider) {
      setMessages((m) => [...m, { role: "system", content: "No provider configured. Add one in the Providers page first.", error: "no-provider" }]);
      return;
    }
    const userMsg = input.trim();
    setInput("");
    setMessages((m) => [...m, { role: "user", content: userMsg }]);
    setSending(true);
    try {
      const res = await api.llmComplete({
        user: userMsg,
        system: systemPrompt || undefined,
      });
      setMessages((m) => [...m, {
        role: "assistant",
        content: res.text,
        provider: res.provider,
        model: res.model,
        elapsedMs: res.elapsed_ms,
      }]);
    } catch (e: any) {
      setMessages((m) => [...m, { role: "assistant", content: e.message, error: e.message }]);
    } finally {
      setSending(false);
    }
  };

  return (
    <div className="space-y-6 h-[calc(100vh-4rem)] flex flex-col">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold flex items-center gap-2">
            <Sparkles className="w-7 h-7 text-accent" />
            Chat
          </h1>
          <p className="text-fg-muted mt-1">
            {defaultProvider ? `Using: ${defaultProvider}` : "No provider configured"}
          </p>
        </div>
        <button className="btn-secondary" onClick={() => setShowSystem(!showSystem)}>
          System prompt
        </button>
      </div>

      {showSystem && (
        <div className="glass-panel p-4">
          <label className="text-xs uppercase tracking-wider text-fg-muted mb-2 block">System prompt</label>
          <textarea
            className="input font-mono text-xs min-h-[80px]"
            value={systemPrompt}
            onChange={(e) => setSystemPrompt(e.target.value)}
          />
        </div>
      )}

      {/* Messages */}
      <div className="flex-1 glass-panel p-6 overflow-y-auto space-y-4">
        {messages.length === 0 ? (
          <div className="text-center py-12">
            <Sparkles className="w-12 h-12 mx-auto text-fg-subtle mb-4" />
            <p className="text-fg-muted">Start a conversation with your AI</p>
          </div>
        ) : (
          messages.map((m, i) => <MessageBubble key={i} msg={m} />)
        )}
        {sending && (
          <div className="flex items-center gap-3 text-fg-muted">
            <div className="w-8 h-8 rounded-full bg-accent/20 flex items-center justify-center">
              <Cpu className="w-4 h-4 text-accent animate-pulse" />
            </div>
            <span className="text-sm">Thinking…</span>
          </div>
        )}
        <div ref={endRef} />
      </div>

      {/* Input */}
      <div className="glass-panel p-4">
        <div className="flex items-end gap-3">
          <textarea
            className="input flex-1 min-h-[48px] max-h-32 resize-none"
            placeholder="Message your AI…"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                send();
              }
            }}
            rows={1}
          />
          <button className="btn-primary h-12 px-4" onClick={send} disabled={!input.trim() || sending}>
            <Send className="w-4 h-4" />
          </button>
        </div>
        <p className="text-xs text-fg-subtle mt-2">
          Press Enter to send · Shift+Enter for new line
        </p>
      </div>
    </div>
  );
}

function MessageBubble({ msg }: { msg: Message }) {
  const isUser = msg.role === "user";
  const isError = !!msg.error;
  const isSystem = msg.role === "system";

  if (isSystem) {
    return (
      <div className="flex items-start gap-3 justify-center">
        <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-warning/10 border border-warning/30 text-warning text-xs">
          <AlertCircle className="w-3.5 h-3.5" />
          {msg.content}
        </div>
      </div>
    );
  }

  return (
    <div className={cn("flex items-start gap-3", isUser && "flex-row-reverse")}>
      <div className={cn(
        "w-8 h-8 rounded-full flex items-center justify-center flex-shrink-0",
        isUser ? "bg-blue-500/20" : "bg-accent/20"
      )}>
        {isUser ? <User className="w-4 h-4 text-blue-400" /> : <Cpu className="w-4 h-4 text-accent" />}
      </div>
      <div className={cn("max-w-[75%]", isUser && "text-right")}>
        <div className={cn(
          "rounded-2xl px-4 py-3",
          isUser ? "bg-blue-500/10 border border-blue-500/20" : isError ? "bg-danger/10 border border-danger/30" : "bg-bg-elevated border border-border"
        )}>
          <p className="text-sm text-fg whitespace-pre-wrap break-words">{msg.content}</p>
        </div>
        {!isUser && !isError && (
          <div className="text-[11px] text-fg-subtle mt-1.5 flex items-center gap-2">
            <span className="font-mono">{msg.provider}</span>
            <span>·</span>
            <span className="font-mono">{msg.model}</span>
            {msg.elapsedMs && (
              <>
                <span>·</span>
                <span>{msg.elapsedMs}ms</span>
              </>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
