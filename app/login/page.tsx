"use client";

import { useState, type FormEvent } from "react";
import { useRouter } from "next/navigation";
import { Loader2, Shield } from "lucide-react";
import { loginWithCredentials } from "@/lib/lango/api-client";
import { saveSession } from "@/lib/lango/session";

/// Real login (chat feature, Phase 4). This dashboard's other views keep
/// authenticating transparently as a fixed demo account (see
/// lib/lango/api-client.ts) — that convenience is untouched. This page
/// exists specifically so a real user's role can drive real routing: a
/// staff-role login lands on /chat (staff has no dashboard access in this
/// product's role model), while compliance_admin/department_reviewer land
/// on the dashboard, which itself now has a real link to /chat too. No new
/// auth mechanism — this calls the exact same POST /api/auth/login the
/// demo flow already uses.
export default function LoginPage() {
  const router = useRouter();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setError(null);
    if (!email.trim() || !password) {
      setError("Enter both an email and a password.");
      return;
    }
    setSubmitting(true);
    try {
      const { token, user } = await loginWithCredentials(email.trim(), password);
      saveSession(token, user);
      router.push(user.role === "staff" ? "/chat" : "/");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="min-h-screen w-full bg-[#F6F7F8] text-[#14171C] flex items-center justify-center font-sans p-4">
      <div className="w-full max-w-sm">
        <div className="flex items-center gap-2 justify-center mb-1">
          <Shield size={20} className="text-[#8A6323]" />
          <span className="font-semibold tracking-wide text-lg">LANGO</span>
        </div>
        <p className="text-center text-[10px] text-[#8A93A1] tracking-wide mb-6">AI DATA GUARD</p>

        <form
          onSubmit={handleSubmit}
          className="bg-[#FFFFFF] border border-[#E1E4E8] rounded-md p-5 space-y-4"
        >
          <div>
            <label htmlFor="login-email" className="block text-xs text-[#8A93A1] mb-1">
              Email
            </label>
            <input
              id="login-email"
              type="email"
              autoComplete="username"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              className="w-full bg-[#F6F7F8] border border-[#E1E4E8] text-[#14171C] text-sm rounded px-3 py-2"
            />
          </div>
          <div>
            <label htmlFor="login-password" className="block text-xs text-[#8A93A1] mb-1">
              Password
            </label>
            <input
              id="login-password"
              type="password"
              autoComplete="current-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              className="w-full bg-[#F6F7F8] border border-[#E1E4E8] text-[#14171C] text-sm rounded px-3 py-2"
            />
          </div>

          {error && (
            <p className="text-xs text-[#A83A3A] bg-[#A83A3A1A] border border-[#A83A3A55] rounded px-3 py-2">
              {error}
            </p>
          )}

          <button
            type="submit"
            disabled={submitting}
            className="w-full flex items-center justify-center gap-2 bg-[#14171C] text-white text-sm rounded px-4 py-2 hover:bg-[#2A2E36] disabled:opacity-50"
          >
            {submitting && <Loader2 size={14} className="animate-spin" />}
            {submitting ? "Signing in…" : "Sign in"}
          </button>
        </form>

        <p className="text-center text-[10px] text-[#8A93A1] mt-4 leading-relaxed">
          Regulated institution demo instance. No raw prompts stored.
        </p>
      </div>
    </div>
  );
}
