"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { LayoutDashboard, LogOut, Shield } from "lucide-react";
import { ChatView } from "@/components/lango/chat-view";
import { clearSession, getSession, type LangoUser } from "@/lib/lango/session";

/// /chat (chat feature, Phase 4). A real Next.js route, not another
/// client-side view inside LangoDashboard's sidebar switch — see
/// Questions.md for why: it needs its own URL a staff-role login can land
/// on directly (staff has no dashboard access at all in this product's
/// existing role model), and its own reachable-but-distinct entry point for
/// compliance_admin/department_reviewer, who can use both this and the
/// existing dashboard.
export default function ChatPage() {
  const [user, setUser] = useState<LangoUser | null>(null);

  useEffect(() => {
    setUser(getSession()?.user ?? null);
  }, []);

  return (
    <div className="min-h-screen w-full bg-[#F6F7F8] text-[#14171C] flex flex-col font-sans">
      <header className="px-4 md:px-8 py-3 border-b border-[#E1E4E8] flex items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          <Shield size={18} className="text-[#8A6323]" />
          <span className="font-semibold tracking-wide">LANGO</span>
          <span className="text-xs text-[#8A93A1] ml-1">Chat</span>
        </div>
        <div className="flex items-center gap-3">
          {/* Only shown for a role that actually has dashboard access — a
              staff-role user never sees a link back to a dashboard they
              can't open (matching auth::require_role's existing server-side
              rule, mirrored here so the UI doesn't dangle a link that would
              just 403). No real session at all (the demo/mock viewing path)
              is treated the same as compliance_admin, since the demo
              account genuinely is one. */}
          {(!user || user.role !== "staff") && (
            <Link
              href="/"
              className="flex items-center gap-1.5 text-xs text-[#5B6270] hover:text-[#14171C]"
            >
              <LayoutDashboard size={14} /> Dashboard
            </Link>
          )}
          {user && (
            <button
              type="button"
              onClick={() => {
                clearSession();
                window.location.href = "/login";
              }}
              className="flex items-center gap-1.5 text-xs text-[#5B6270] hover:text-[#14171C]"
            >
              <LogOut size={14} /> Log out
            </button>
          )}
        </div>
      </header>
      <main className="flex-1 min-w-0 p-4 md:p-8">
        <ChatView />
      </main>
    </div>
  );
}
