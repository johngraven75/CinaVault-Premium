import test from "node:test";
import assert from "node:assert/strict";

import {
  createRemoteAccountFormState,
  dismissRemoteAccountFormStatus,
  failRemoteAccountSave,
  finishRemoteAccountSave,
  markRemoteAccessKeyCopied,
  prepareRemoteAccountSave,
  revealRemoteAccessKey,
  updateRemoteAccountFormField,
} from "../src/utils/remoteAccessAccountForm.ts";
import { rotateRemoteAccessUserKey } from "../src/utils/remoteAccessUserCommands.ts";

function validFormState() {
  let state = createRemoteAccountFormState();
  state = updateRemoteAccountFormField(state, "displayName", "Home Viewer");
  state = updateRemoteAccountFormField(state, "email", " Viewer@Example.com ");
  state = updateRemoteAccountFormField(state, "password", "CorrectHorse42!");
  return state;
}

test("remote user save reports inline validation errors before saving", () => {
  let state = createRemoteAccountFormState();
  state = updateRemoteAccountFormField(state, "email", "not-an-email");
  state = updateRemoteAccountFormField(state, "password", "short");

  const prepared = prepareRemoteAccountSave(state);

  assert.deepEqual(prepared.errors, {
    email: "Enter a valid email address.",
    password: "Use at least 8 characters.",
  });
  assert.deepEqual(prepared.status, {
    kind: "error",
    message: "Fix the highlighted fields and try again.",
  });
});

test("remote user failure retains non-secret fields and remains until dismissed", () => {
  const prepared = prepareRemoteAccountSave(validFormState());
  assert.equal(prepared.status.kind, "saving");

  const failed = failRemoteAccountSave(
    prepared,
    "A remote user with this email already exists.",
  );

  assert.deepEqual(failed.fields, {
    displayName: "Home Viewer",
    email: " Viewer@Example.com ",
    password: "",
  });
  assert.deepEqual(failed.status, {
    kind: "error",
    message: "A remote user with this email already exists.",
  });
  assert.equal(
    updateRemoteAccountFormField(failed, "email", "other@example.com").status
      .kind,
    "error",
  );
  assert.equal(dismissRemoteAccountFormStatus(failed).status.kind, "idle");
});

test("remote user success clears the form and key reveal tracks copy state", () => {
  const saved = finishRemoteAccountSave(
    prepareRemoteAccountSave(validFormState()),
    "viewer@example.com",
  );

  assert.deepEqual(saved.fields, {
    displayName: "",
    email: "",
    password: "",
  });
  assert.deepEqual(saved.status, {
    kind: "success",
    message: "Remote user viewer@example.com created.",
  });

  const revealed = revealRemoteAccessKey(
    "viewer@example.com",
    "cvra_one_time_secret",
    "created",
  );
  assert.equal(revealed.copied, false);
  assert.equal(markRemoteAccessKeyCopied(revealed).copied, true);
});

test("remote user rotation uses the explicit access-key command", async () => {
  const calls = [];
  const rotated = await rotateRemoteAccessUserKey(
    async (command, args) => {
      calls.push({ command, args });
      return {
        email: "viewer@example.com",
        access_key: "cvra_replacement_secret",
        access_key_preview: "t_secret",
      };
    },
    "viewer@example.com",
  );

  assert.deepEqual(calls, [
    {
      command: "rotate_remote_access_key",
      args: { email: "viewer@example.com" },
    },
  ]);
  assert.equal(rotated.access_key, "cvra_replacement_secret");
});
