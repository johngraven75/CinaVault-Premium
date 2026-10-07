// Global CinaVault Plus modal. Loads entitlements once at boot and shows the
// PaywallPanel whenever a gate or a "PAYWALL:" command refusal asks for it.
import { useEffect } from "react";
import type { JSX } from "react";
import { AnimatePresence, motion } from "framer-motion";
import PaywallPanel from "./PaywallPanel";
import { useEntitlementsStore } from "../../services/entitlements";

export default function PaywallHost(): JSX.Element {
  const prompt = useEntitlementsStore((state) => state.paywallPrompt);
  const dismiss = useEntitlementsStore((state) => state.dismissPaywall);
  const refresh = useEntitlementsStore((state) => state.refresh);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    if (!prompt) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") dismiss();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [prompt, dismiss]);

  return (
    <AnimatePresence>
      {prompt && (
        <motion.div
          key="cv-paywall"
          className="fixed inset-0 z-[140] flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm"
          role="dialog"
          aria-modal="true"
          aria-label="CinaVault Plus"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          onClick={(event) => {
            if (event.target === event.currentTarget) dismiss();
          }}
        >
          <div className="max-h-[90vh] w-full max-w-xl overflow-y-auto">
            <PaywallPanel
              feature={prompt.feature}
              message={prompt.message}
              onClose={dismiss}
              onActivated={() => window.setTimeout(dismiss, 1200)}
            />
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
