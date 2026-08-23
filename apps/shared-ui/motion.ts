import type { EasingFunction, TransitionConfig } from "svelte/transition";

export type SurfaceMotionOptions = {
  duration?: number;
  exitDuration?: number;
  offsetY?: number;
  offsetUnit?: "px" | "%";
  startScale?: number;
  easing?: EasingFunction;
};

function cubicBezier(x1: number, y1: number, x2: number, y2: number): EasingFunction {
  const sampleCurveX = (t: number): number => {
    const inv = 1 - t;
    return 3 * inv * inv * t * x1 + 3 * inv * t * t * x2 + t * t * t;
  };
  const sampleCurveY = (t: number): number => {
    const inv = 1 - t;
    return 3 * inv * inv * t * y1 + 3 * inv * t * t * y2 + t * t * t;
  };

  return (progress: number): number => {
    if (progress <= 0) return 0;
    if (progress >= 1) return 1;
    let low = 0;
    let high = 1;
    for (let iteration = 0; iteration < 12; iteration += 1) {
      const midpoint = (low + high) / 2;
      if (sampleCurveX(midpoint) < progress) low = midpoint;
      else high = midpoint;
    }
    return sampleCurveY((low + high) / 2);
  };
}

export const modalSurfaceEasing = cubicBezier(.22, .8, .28, 1);
export const toastSurfaceEasing = cubicBezier(.16, 1, .3, 1);
export const inlineNoticeSurfaceEasing = cubicBezier(.23, 1, .32, 1);

export function surfaceMotion(node: Element, options: SurfaceMotionOptions = {}): TransitionConfig {
  const duration = options.duration ?? 220;
  const exitDuration = options.exitDuration ?? duration;
  const offsetY = options.offsetY ?? 8;
  const offsetUnit = options.offsetUnit ?? "px";
  const startScale = options.startScale ?? .985;
  const easing = options.easing ?? modalSurfaceEasing;
  const reducedMotion = (): boolean => typeof window !== "undefined"
    && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const transitionEasing: EasingFunction = (progress) =>
    reducedMotion() ? progress : easing(progress);
  let transitionPhase: "in" | "out" = "in";
  // Keep one reversible transition while selecting the duration at each lifecycle start.
  const getDuration = (): number => reducedMotion()
    ? 120
    : transitionPhase === "out" ? exitDuration : duration;
  node.addEventListener("introstart", () => transitionPhase = "in");
  node.addEventListener("outrostart", () => transitionPhase = "out");

  const transition: TransitionConfig = {
    easing: transitionEasing,
    css: (t, u) => {
      if (reducedMotion()) return `will-change: transform, opacity; opacity: ${t}; transform: translate3d(0, 0, 0) scale(1);`;
      const translateY = `${u * offsetY}${offsetUnit}`;
      const scale = startScale + (1 - startScale) * t;
      return `will-change: transform, opacity; opacity: ${t}; transform: translate3d(0, ${translateY}, 0) scale(${scale});`;
    },
  };
  Object.defineProperty(transition, "duration", { enumerable: true, get: getDuration });
  return transition;
}
