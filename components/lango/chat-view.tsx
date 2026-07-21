"use client";

import { useEffect, useRef, useState } from "react";
import { AlertTriangle, Ban, Flag, Loader2, Plus, SendHorizonal } from "lucide-react";
import {
  fetchChatConversations,
  fetchChatMessages,
  isLiveBackendConfigured,
  sendChatMessage,
  type ChatConversationSummary,
  type ChatMessage,
} from "@/lib/lango/api-client";
import { UnavailableNotice } from "./atoms";

/// Native chat UI (chat feature, Phase 4). Reuses this dashboard's existing
/// amber/red/green status grammar (see decision-badge.ts) rather than
/// inventing a new one: a response flagged after the fact gets the same
/// orange (#C2660C) "needs attention but isn't blocked" treatment
/// `redacted_low_confidence_review` already uses elsewhere, since it's the
/// same underlying idea — nothing was stopped, a human should take a look.
///
/// A blocked prompt is shown as a transient banner, not a chat bubble — the
/// backend creates no chat_messages row for it (see routes/chat.rs), so
/// there is nothing to persist or redisplay on a later visit; this mirrors
/// exactly how the browser extension's own block banner works.
const POLL_INTERVAL_MS = 2000;
const POLL_MAX_ATTEMPTS = 10; // ~20s — long enough for the background response scan to finish in the normal case

export function ChatView() {
  const live = isLiveBackendConfigured();

  const [conversations, setConversations] = useState<ChatConversationSummary[]>([]);
  const [activeConversationId, setActiveConversationId] = useState<string | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const [streamingText, setStreamingText] = useState<string | null>(null);
  const [blockedNotice, setBlockedNotice] = useState<{ userMessage: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loadingConversations, setLoadingConversations] = useState(true);

  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!live) {
      setLoadingConversations(false);
      return;
    }
    let cancelled = false;
    fetchChatConversations()
      .then((list) => {
        if (!cancelled) setConversations(list);
      })
      .catch((err) => {
        if (!cancelled) setError(err instanceof Error ? err.message : String(err));
      })
      .finally(() => {
        if (!cancelled) setLoadingConversations(false);
      });
    return () => {
      cancelled = true;
    };
  }, [live]);

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: "smooth" });
  }, [messages, streamingText]);

  async function openConversation(id: string) {
    setActiveConversationId(id);
    setBlockedNotice(null);
    setError(null);
    try {
      const msgs = await fetchChatMessages(id);
      setMessages(msgs);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  function startNewConversation() {
    setActiveConversationId(null);
    setMessages([]);
    setBlockedNotice(null);
    setError(null);
  }

  async function pollForResponseFlag(conversationId: string, assistantMessageId: string) {
    for (let attempt = 0; attempt < POLL_MAX_ATTEMPTS; attempt++) {
      await new Promise((resolve) => setTimeout(resolve, POLL_INTERVAL_MS));
      try {
        const fresh = await fetchChatMessages(conversationId);
        const match = fresh.find((m) => m.id === assistantMessageId);
        if (match && match.responseFlagged !== null) {
          setMessages(fresh);
          return;
        }
      } catch {
        // A transient poll failure just means the flag (if any) shows up a
        // little later on the next successful poll — not worth surfacing
        // as an error for a purely retroactive, best-effort warning.
      }
    }
  }

  async function handleSend() {
    const message = input.trim();
    if (!message || sending) return;
    setError(null);
    setBlockedNotice(null);
    setSending(true);
    setStreamingText("");
    setInput("");

    const optimisticUserMessage: ChatMessage = {
      id: `pending-${Date.now()}`,
      role: "user",
      content: message,
      riskScore: null,
      decision: null,
      responseFlagged: null,
      createdAt: new Date().toISOString(),
    };
    setMessages((prev) => [...prev, optimisticUserMessage]);

    try {
      const result = await sendChatMessage(activeConversationId, message, (delta) => {
        setStreamingText((prev) => (prev ?? "") + delta);
      });

      if (result.blocked) {
        // Nothing was sent — remove the optimistic bubble, matching the
        // backend's "no chat_messages row for a blocked turn" design.
        setMessages((prev) => prev.filter((m) => m.id !== optimisticUserMessage.id));
        setBlockedNotice({ userMessage: result.userMessage });
        setStreamingText(null);
        return;
      }

      const conversationId = result.meta.conversationId;
      setActiveConversationId(conversationId);
      if (!conversations.some((c) => c.id === conversationId)) {
        setConversations((prev) => [{ id: conversationId, title: null, createdAt: new Date().toISOString() }, ...prev]);
      }

      // Refresh from the server for authoritative content (the user's
      // message as actually redacted and stored — see
      // lib/lango/api-client.ts's own comment on why this can legitimately
      // differ from what was optimistically shown) and the real assistant
      // message id needed to poll for a retroactive flag.
      //
      // Real race, found by actually running this (not by inspection): the
      // HTTP stream this promise was awaiting closes the instant the
      // backend forwards its last chunk (routes/chat.rs's stream_and_scan
      // drops the client channel BEFORE running the response scan and
      // inserting the assistant chat_messages row, deliberately, so scan
      // latency never delays what the user sees). That means a refresh
      // fired immediately after the stream ends can genuinely beat the
      // backend's own background INSERT — the fetched list would have the
      // user's turn but not yet the assistant's, and clearing
      // streamingText right then would blank a reply the user just
      // finished watching stream in. Retried, bounded, rather than a
      // single fetch-and-clear.
      let fresh = await fetchChatMessages(conversationId);
      for (let attempt = 0; attempt < 5 && !fresh.some((m) => m.role === "assistant"); attempt++) {
        await new Promise((resolve) => setTimeout(resolve, 300));
        fresh = await fetchChatMessages(conversationId);
      }
      setMessages(fresh);
      setStreamingText(null);

      const lastAssistant = [...fresh].reverse().find((m) => m.role === "assistant");
      if (lastAssistant && lastAssistant.responseFlagged === null) {
        pollForResponseFlag(conversationId, lastAssistant.id);
      }
    } catch (err) {
      setMessages((prev) => prev.filter((m) => m.id !== optimisticUserMessage.id));
      setStreamingText(null);
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSending(false);
    }
  }

  if (!live) {
    return (
      <div className="max-w-2xl">
        <UnavailableNotice />
      </div>
    );
  }

  return (
    <div className="flex h-[calc(100vh-8.5rem)] md:h-[calc(100vh-6.5rem)] gap-4">
      <aside className="hidden sm:flex w-56 shrink-0 flex-col border border-[#E1E4E8] rounded-md bg-[#FFFFFF] overflow-hidden">
        <div className="p-3 border-b border-[#E1E4E8]">
          <button
            type="button"
            onClick={startNewConversation}
            className="w-full flex items-center justify-center gap-1.5 bg-[#14171C] text-white text-xs rounded px-3 py-1.5 hover:bg-[#2A2E36]"
          >
            <Plus size={13} /> New chat
          </button>
        </div>
        <div className="flex-1 overflow-y-auto">
          {loadingConversations && (
            <p className="text-xs text-[#8A93A1] p-3 flex items-center gap-2">
              <Loader2 size={12} className="animate-spin" /> Loading…
            </p>
          )}
          {!loadingConversations && conversations.length === 0 && (
            <p className="text-xs text-[#8A93A1] p-3">No conversations yet.</p>
          )}
          {conversations.map((c) => (
            <button
              key={c.id}
              type="button"
              onClick={() => openConversation(c.id)}
              className={`w-full text-left px-3 py-2 text-xs border-l-2 truncate ${
                activeConversationId === c.id
                  ? "bg-[#F0F1F3] text-[#14171C] border-[#8A6323]"
                  : "text-[#5B6270] border-transparent hover:text-[#14171C]"
              }`}
            >
              {c.title ?? new Date(c.createdAt).toLocaleString()}
            </button>
          ))}
        </div>
      </aside>

      <div className="flex-1 min-w-0 flex flex-col border border-[#E1E4E8] rounded-md bg-[#FFFFFF] overflow-hidden">
        <div ref={scrollRef} className="flex-1 overflow-y-auto p-4 space-y-3">
          {messages.length === 0 && !streamingText && (
            <p className="text-sm text-[#8A93A1] text-center mt-8">
              Send a message to start. Every message is scanned the same way as the browser extension — sensitive
              details are redacted automatically before this leaves your organisation.
            </p>
          )}
          {messages.map((m) => (
            <MessageBubble key={m.id} message={m} />
          ))}
          {streamingText !== null && (
            <div className="flex justify-start">
              <div className="max-w-[80%] rounded-md px-3 py-2 text-sm bg-[#F6F7F8] text-[#14171C] whitespace-pre-wrap">
                {streamingText}
                <span className="inline-block w-1.5 h-3.5 bg-[#8A93A1] ml-0.5 animate-pulse motion-reduce:animate-none align-middle" />
              </div>
            </div>
          )}
        </div>

        {blockedNotice && (
          <div className="mx-4 mb-2 flex items-start gap-2 text-sm text-[#A83A3A] bg-[#A83A3A1A] border border-[#A83A3A55] rounded-md p-3">
            <Ban size={16} className="mt-0.5 shrink-0" />
            <p>{blockedNotice.userMessage}</p>
          </div>
        )}
        {error && (
          <div className="mx-4 mb-2 flex items-start gap-2 text-sm text-[#A83A3A] bg-[#A83A3A1A] border border-[#A83A3A55] rounded-md p-3">
            <AlertTriangle size={16} className="mt-0.5 shrink-0" />
            <p>{error}</p>
          </div>
        )}

        <div className="border-t border-[#E1E4E8] p-3 flex items-end gap-2">
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                handleSend();
              }
            }}
            placeholder="Message Lango…"
            rows={1}
            disabled={sending}
            className="flex-1 resize-none bg-[#F6F7F8] border border-[#E1E4E8] text-[#14171C] text-sm rounded px-3 py-2 max-h-32 disabled:opacity-60"
          />
          <button
            type="button"
            onClick={handleSend}
            disabled={sending || !input.trim()}
            aria-label="Send message"
            className="shrink-0 bg-[#14171C] text-white rounded p-2.5 hover:bg-[#2A2E36] disabled:opacity-50"
          >
            {sending ? <Loader2 size={16} className="animate-spin" /> : <SendHorizonal size={16} />}
          </button>
        </div>
      </div>
    </div>
  );
}

function MessageBubble({ message }: { message: ChatMessage }) {
  const isUser = message.role === "user";
  return (
    <div className={`flex ${isUser ? "justify-end" : "justify-start"}`}>
      <div className="max-w-[80%]">
        <div
          className={`rounded-md px-3 py-2 text-sm whitespace-pre-wrap ${
            isUser ? "bg-[#14171C] text-white" : "bg-[#F6F7F8] text-[#14171C]"
          }`}
        >
          {message.content}
        </div>
        {/* Same amber/orange "needs attention, not blocked" grammar
            decision-badge.ts already uses for redacted_low_confidence_review
            — a retroactive response flag is the same underlying idea: nothing
            was stopped, a human should take a look. */}
        {message.responseFlagged === true && (
          <div className="mt-1 flex items-start gap-1.5 text-xs text-[#C2660C] bg-[#C2660C1A] border border-[#C2660C55] rounded px-2 py-1.5">
            <Flag size={13} className="mt-0.5 shrink-0" />
            <p>This response may contain sensitive information — review it carefully before using or sharing it.</p>
          </div>
        )}
        {isUser && message.decision === "redacted_low_confidence_review" && (
          <div className="mt-1 flex items-start gap-1.5 text-xs text-[#C2660C] bg-[#C2660C1A] border border-[#C2660C55] rounded px-2 py-1.5">
            <Flag size={13} className="mt-0.5 shrink-0" />
            <p>Sent with a low-confidence redaction, flagged for compliance review.</p>
          </div>
        )}
      </div>
    </div>
  );
}
