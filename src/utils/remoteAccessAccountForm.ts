export type RemoteAccountFormFields = {
  displayName: string;
  email: string;
  password: string;
};

export type RemoteAccountFormErrors = Partial<
  Record<"email" | "password", string>
>;

export type RemoteAccountFormStatus =
  | { kind: "idle" }
  | { kind: "saving"; message: string }
  | { kind: "success"; message: string }
  | { kind: "error"; message: string };

export type RemoteAccountFormState = {
  fields: RemoteAccountFormFields;
  errors: RemoteAccountFormErrors;
  status: RemoteAccountFormStatus;
};

export type RemoteAccessKeyReveal = {
  email: string;
  accessKey: string;
  action: "created" | "rotated";
  copied: boolean;
};

const emptyFields = (): RemoteAccountFormFields => ({
  displayName: "",
  email: "",
  password: "",
});

export function createRemoteAccountFormState(): RemoteAccountFormState {
  return { fields: emptyFields(), errors: {}, status: { kind: "idle" } };
}

export function updateRemoteAccountFormField(
  state: RemoteAccountFormState,
  field: keyof RemoteAccountFormFields,
  value: string,
): RemoteAccountFormState {
  const errors = { ...state.errors };
  if (field === "email" || field === "password") delete errors[field];
  return {
    ...state,
    fields: { ...state.fields, [field]: value },
    errors,
  };
}

export function validateRemoteAccountForm(
  fields: RemoteAccountFormFields,
): RemoteAccountFormErrors {
  const errors: RemoteAccountFormErrors = {};
  if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(fields.email.trim())) {
    errors.email = "Enter a valid email address.";
  }
  if (fields.password.length < 8) {
    errors.password = "Use at least 8 characters.";
  }
  return errors;
}

export function prepareRemoteAccountSave(
  state: RemoteAccountFormState,
): RemoteAccountFormState {
  const errors = validateRemoteAccountForm(state.fields);
  if (Object.keys(errors).length > 0) {
    return {
      ...state,
      errors,
      status: {
        kind: "error",
        message: "Fix the highlighted fields and try again.",
      },
    };
  }
  return {
    ...state,
    errors: {},
    status: { kind: "saving", message: "Saving remote user…" },
  };
}

export function finishRemoteAccountSave(
  state: RemoteAccountFormState,
  normalizedEmail: string,
): RemoteAccountFormState {
  return {
    ...state,
    fields: emptyFields(),
    errors: {},
    status: {
      kind: "success",
      message: `Remote user ${normalizedEmail} created.`,
    },
  };
}

export function failRemoteAccountSave(
  state: RemoteAccountFormState,
  message: string,
): RemoteAccountFormState {
  return {
    ...state,
    fields: { ...state.fields, password: "" },
    status: { kind: "error", message },
  };
}

export function setRemoteAccountFormStatus(
  state: RemoteAccountFormState,
  status: Exclude<RemoteAccountFormStatus, { kind: "idle" | "saving" }>,
): RemoteAccountFormState {
  return { ...state, status };
}

export function dismissRemoteAccountFormStatus(
  state: RemoteAccountFormState,
): RemoteAccountFormState {
  return { ...state, status: { kind: "idle" } };
}

export function revealRemoteAccessKey(
  email: string,
  accessKey: string,
  action: RemoteAccessKeyReveal["action"],
): RemoteAccessKeyReveal {
  return { email, accessKey, action, copied: false };
}

export function markRemoteAccessKeyCopied(
  reveal: RemoteAccessKeyReveal,
): RemoteAccessKeyReveal {
  return { ...reveal, copied: true };
}
