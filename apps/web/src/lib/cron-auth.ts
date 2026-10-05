import { timingSafeEqual } from "node:crypto";

export function isAuthorizedCronRequest(
  authorization: string | null,
  secret: string | undefined,
): boolean {
  if (!secret || secret.length < 32 || !authorization) return false;
  const expected = Buffer.from("Bearer " + secret);
  const presented = Buffer.from(authorization);
  return (
    expected.length === presented.length &&
    timingSafeEqual(expected, presented)
  );
}
