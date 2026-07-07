/**
 * Submit handler for /admin/signin, factored out so it can be unit-tested
 * without jsdom. The page wires this to its <form> onSubmit; the handler
 * MUST clear the input value (success or failure) and never persist the
 * token in any caller-accessible state.
 *
 * Returns a discriminated union the caller maps to router push (on ok) or
 * generic error display (on failure).
 */
export interface SubmitDeps {
  readonly setAdminToken: (token: string) => Promise<boolean>;
}

export type SubmitResult =
  | { ok: true }
  | { ok: false; error: string };

const GENERIC_ERROR = 'Sign-in failed. Verify the token and try again.';

/**
 * Reads the token from the input, calls setAdminToken, clears the input
 * value, and returns the result. The input is cleared BEFORE the network
 * call returns so a slow upstream cannot leave the secret visible in the
 * DOM longer than one tick.
 */
export async function submitAdminToken(
  input: { value: string },
  deps: SubmitDeps,
): Promise<SubmitResult> {
  const token = input.value;
  input.value = '';
  let ok = false;
  try {
    ok = await deps.setAdminToken(token);
  } catch {
    ok = false;
  }
  if (ok) return { ok: true };
  return { ok: false, error: GENERIC_ERROR };
}
