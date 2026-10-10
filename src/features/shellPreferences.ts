// Applies the app-wide look switches to the document: glass panels, compact
// layout, poster hover, motion, backdrop opacity and the user's custom CSS.
// The pure part (shellPreferences) is unit tested; the hook only writes it out.
import { useEffect } from "react";

import { featureConfig, isFeatureOn, type FeatureSettings } from "./featureState.ts";

export interface ShellPreferences {
  classes: Record<string, boolean>;
  backdropOpacity: number;
  customCss: string;
  reduceMotion: boolean;
}

export const CUSTOM_CSS_LIMIT = 64 * 1024;

/** Custom CSS without anything that could load remote code or break out of the style element. */
export function sanitizeCustomCss(css: unknown): string {
  if (typeof css !== "string") return "";
  return css
    .slice(0, CUSTOM_CSS_LIMIT)
    .replace(/<\/?style/gi, "")
    .replace(/@import[^;]*;?/gi, "")
    .replace(/expression\s*\(/gi, "")
    .replace(/url\(['"\s]*(?:https?:|javascript:)[^)]*\)/gi, "none");
}

export function shellPreferences(settings: Record<string, string>, features: FeatureSettings): ShellPreferences {
  const opacity = Number(settings.window_opacity ?? 100);
  const reduceMotion = settings.motion_enabled === "false";
  return {
    classes: {
      "cv-no-glass": !isFeatureOn(features, "glass_effects"),
      "cv-compact": isFeatureOn(features, "compact_mode"),
      "cv-no-poster-hover": !isFeatureOn(features, "poster_hover"),
      "cv-reduce-motion": reduceMotion,
    },
    backdropOpacity: Number.isFinite(opacity) ? Math.min(100, Math.max(60, opacity)) / 100 : 1,
    customCss: isFeatureOn(features, "custom_css")
      ? sanitizeCustomCss(featureConfig(features, "custom_css", { css: "" }).css)
      : "",
    reduceMotion,
  };
}

const STYLE_ID = "cinavault-custom-css";

export function useShellPreferences(settings: Record<string, string>, features: FeatureSettings): ShellPreferences {
  const prefs = shellPreferences(settings, features);
  const classKey = JSON.stringify(prefs.classes);

  useEffect(() => {
    const root = document.documentElement;
    for (const [name, on] of Object.entries(prefs.classes)) root.classList.toggle(name, on);
    root.style.setProperty("--cv-backdrop-opacity", String(prefs.backdropOpacity));
  }, [classKey, prefs.backdropOpacity]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    let style = document.getElementById(STYLE_ID) as HTMLStyleElement | null;
    if (!prefs.customCss) {
      style?.remove();
      return;
    }
    if (!style) {
      style = document.createElement("style");
      style.id = STYLE_ID;
      document.head.appendChild(style);
    }
    style.textContent = prefs.customCss;
  }, [prefs.customCss]);

  return prefs;
}
