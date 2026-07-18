import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");

test("Casting is a dedicated navigation page", () => {
  const store = read("src/store/appStore.ts");
  const sidebar = read("src/components/Sidebar.tsx");
  const app = read("src/App.tsx");
  assert.match(store, /\| "casting"/);
  assert.match(sidebar, /id: "casting"/);
  assert.match(sidebar, /label: "Casting"/);
  assert.match(app, /CastingTab/