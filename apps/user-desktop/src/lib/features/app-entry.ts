import type { UserApi } from "../adapters/api";

export type AppEntryPhase = "idle" | "resizing" | "preparing" | "expanding" | "playing";

export interface AppEntryFeatureState {
  appEntryPhase: AppEntryPhase;
  appMemoryBytes: number | null;
}

export interface AppEntryFeatureDependencies {
  api: Pick<UserApi, "appMemoryBytes">;
  getState: () => AppEntryFeatureState;
  patchState: (patch: Partial<AppEntryFeatureState>) => void;
  setWindowMode: (mode: "app") => Promise<void>;
  tick: () => Promise<void>;
  appWindowExpandDurationMs: number;
}

function waitForStablePaint(): Promise<void> {
  return new Promise((resolve) => {
    window.requestAnimationFrame(() => window.requestAnimationFrame(() => resolve()));
  });
}

function waitForMotion(duration: number): Promise<void> {
  if (duration <= 0 || window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
    return Promise.resolve();
  }
  return new Promise((resolve) => window.setTimeout(resolve, duration));
}

export function createAppEntryFeature(deps: AppEntryFeatureDependencies) {
  let entrySequence = 0;

  async function showApp(): Promise<void> {
    const sequence = ++entrySequence;
    try {
      deps.patchState({ appEntryPhase: "resizing" });
      await deps.tick();
      await waitForStablePaint();
      if (sequence !== entrySequence) return;

      const expandDuration = window.matchMedia("(prefers-reduced-motion: reduce)").matches
        ? 0
        : deps.appWindowExpandDurationMs;
      await deps.setWindowMode("app");
      if (sequence !== entrySequence) return;

      deps.patchState({ appEntryPhase: "preparing" });
      await deps.tick();
      await waitForStablePaint();
      if (sequence !== entrySequence) return;

      deps.patchState({ appEntryPhase: expandDuration > 0 ? "expanding" : "playing" });
      await deps.tick();
      await waitForMotion(expandDuration);
      if (sequence !== entrySequence) return;

      deps.patchState({ appEntryPhase: "playing" });
      if (expandDuration === 0) finishAppEntry();
    } catch {
      if (sequence === entrySequence) deps.patchState({ appEntryPhase: "idle" });
    }
  }

  function finishAppEntry(): void {
    if (deps.getState().appEntryPhase === "playing") {
      deps.patchState({ appEntryPhase: "idle" });
    }
  }

  async function refreshMemory(): Promise<void> {
    const appMemoryBytes = await deps.api.appMemoryBytes().catch(() => null);
    deps.patchState({ appMemoryBytes });
  }

  return { finishAppEntry, refreshMemory, showApp };
}
