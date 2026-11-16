export function classes(...values: Array<string | undefined>) {
  return values.filter(Boolean).join(" ");
}
