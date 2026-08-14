export type RemoteAccessCommandInvoker = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>;

export type RemoteAccessKeyRotation = {
  email: string;
  access_key: string;
  access_key_preview: string;
};

export function rotateRemoteAccessUserKey(
  invokeCommand: RemoteAccessCommandInvoker,
  email: string,
): Promise<RemoteAccessKeyRotation | null> {
  return invokeCommand<RemoteAccessKeyRotation | null>(
    "rotate_remote_access_key",
    { email },
  );
}
