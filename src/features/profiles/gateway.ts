import {
  type AccountError,
  commands,
  type Event,
  events,
  type Profile,
  type ProfileList,
  type ProfileOverview,
  type Result,
} from "@/bindings";

/**
 * Gateway for the profiles (PFL): the only file of this feature calling `commands.*` (F3).
 */
export const profileGateway = {
  // PFL-023 / PFL-012 — every profile with its account count, and the profile in use.
  async listProfiles(): Promise<Result<ProfileList, AccountError>> {
    return await commands.listProfiles();
  },

  // PFL-055 — every profile with its own figures, never added across profiles.
  async getProfileOverview(): Promise<Result<ProfileOverview, AccountError>> {
    return await commands.getProfileOverview();
  },

  // PFL-032 — makes an existing profile the profile in use.
  async setProfileInUse(profileId: string): Promise<Result<null, AccountError>> {
    return await commands.setProfileInUse(profileId);
  },

  // PFL-020 — a new profile.
  async createProfile(name: string): Promise<Result<Profile, AccountError>> {
    return await commands.createProfile(name);
  },

  // PFL-030 — a profile renamed, the default one included.
  async renameProfile(profileId: string, name: string): Promise<Result<Profile, AccountError>> {
    return await commands.renameProfile(profileId, name);
  },

  // PFL-040 — a profile holding no account removed.
  async removeProfile(profileId: string): Promise<Result<null, AccountError>> {
    return await commands.removeProfile(profileId);
  },

  async subscribeToEvents(callback: (type: Event["type"]) => void): Promise<() => void> {
    return events.event.listen((event) => {
      callback(event.payload.type);
    });
  },
};
