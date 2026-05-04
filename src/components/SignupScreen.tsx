import React, { useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Cloud, UserPlus, ShieldCheck, Mail } from "lucide-react";

interface SignupScreenProps {
  onComplete: (payload: { provider: "google" | "microsoft" | "local"; username: string; passwordHash?: string }) => Promise<void>;
}

function passwordValid(password: string): boolean {
  const hasMinLength = password.length >= 8;
  const hasNumber = /\d/.test(password);
  const hasSpecial = /[^A-Za-z0-9]/.test(password);
  return hasMinLength && hasNumber && hasSpecial;
}

async function sha256(input: string): Promise<string> {
  const bytes = new TextEncoder().encode(input);
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(digest)).map((b) => b.toString(16).padStart(2, "0")).join("");
}

export default function SignupScreen({ onComplete }: SignupScreenProps) {
  const [email, setEmail] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  const passwordChecklist = useMemo(() => ({
    length: password.length >= 8,
    number: /\d/.test(password),
    special: /[^A-Za-z0-9]/.test(password),
  }), [password]);

  const finishSocialSignup = async (provider: "google" | "microsoft") => {
    setError("");
    if (!email.trim()) {
      setError("Enter your Google/Microsoft account email first.");
      return;
    }

    setSaving(true);
    try {
      if (provider === "google") {
        await invoke("open_external_url", { url: "https://accounts.google.com/signin" });
      } else {
        await invoke("open_external_url", { url: "https://login.microsoftonline.com/" });
      }
      await onComplete({ provider, username: email.trim() });
    } catch (e) {
      setError(`Signup failed: ${e}`);
    }
    setSaving(false);
  };

  const finishLocalSignup = async () => {
    setError("");

    if (!username.trim()) {
      setError("Username is required.");
      return;
    }

    if (!passwordValid(password)) {
      setError("Password must be at least 8 characters with at least one number and one special symbol.");
      return;
    }

    if (password !== confirmPassword) {
      setError("Passwords do not match.");
      return;
    }

    setSaving(true);
    try {
      const passwordHash = await sha256(password);
      await onComplete({ provider: "local", username: username.trim(), passwordHash });
    } catch (e) {
      setError(`Signup failed: ${e}`);
    }
    setSaving(false);
  };

  return (
    <div className="min-h-screen w-screen flex items-center justify-center px-4" style={{ background: "var(--cv-bg)" }}>
      <div className="glass-panel p-6 w-full max-w-3xl space-y-5">
        <div>
          <h1 className="text-2xl font-bold">Create Your CinaVault Account</h1>
          <p className="text-sm text-cv-subtext mt-1">Choose Google, Microsoft, or local signup before using the app.</p>
        </div>

        <div className="glass-panel-2 p-4 rounded-lg space-y-3">
          <h3 className="text-sm font-semibold flex items-center gap-2"><Mail size={14} className="text-cv-accent" /> Sign up with Google or Microsoft</h3>
          <input
            className="cv-input"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder="Your Google or Microsoft email"
          />
          <div className="flex gap-2">
            <button disabled={saving} onClick={() => finishSocialSignup("google")} className="cv-btn cv-btn-primary text-xs">
              <Cloud size={12} /> Continue with Google
            </button>
            <button disabled={saving} onClick={() => finishSocialSignup("microsoft")} className="cv-btn cv-btn-secondary text-xs">
              <Cloud size={12} /> Continue with Microsoft
            </button>
          </div>
        </div>

        <div className="glass-panel-2 p-4 rounded-lg space-y-3">
          <h3 className="text-sm font-semibold flex items-center gap-2"><UserPlus size={14} className="text-cv-accent" /> Or sign up with username + password</h3>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            <input className="cv-input" value={username} onChange={(e) => setUsername(e.target.value)} placeholder="New username" />
            <input type="password" className="cv-input" value={password} onChange={(e) => setPassword(e.target.value)} placeholder="New password" />
            <input type="password" className="cv-input md:col-span-2" value={confirmPassword} onChange={(e) => setConfirmPassword(e.target.value)} placeholder="Confirm password" />
          </div>
          <div className="text-[11px] text-cv-subtext space-y-1">
            <div className={passwordChecklist.length ? "text-green-400" : ""}>At least 8 characters</div>
            <div className={passwordChecklist.number ? "text-green-400" : ""}>At least one number</div>
            <div className={passwordChecklist.special ? "text-green-400" : ""}>At least one special symbol</div>
          </div>
          <button disabled={saving} onClick={finishLocalSignup} className="cv-btn cv-btn-primary text-xs">
            <ShieldCheck size={12} /> Create Local Account
          </button>
        </div>

        {error && <div className="text-xs text-cv-danger">{error}</div>}
      </div>
    </div>
  );
}
