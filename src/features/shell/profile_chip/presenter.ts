import type { Profile } from "@/bindings";

/**
 * PFL-011 — a profile's name as the chip shows it: the default profile has none until named,
 * and reads as `defaultName` (the window's language). Pure.
 */
export function chipProfileName(profile: Profile | null, defaultName: string): string {
  return profile?.name ?? defaultName;
}
