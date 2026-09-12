/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 baibai and Botting contributors
 *
 * Botting is free software: you can redistribute it and/or modify it under
 * the GNU Affero General Public License version 3, as published by the
 * Free Software Foundation. This program comes WITHOUT ANY WARRANTY;
 * without even the implied warranty of MERCHANTABILITY or FITNESS FOR A
 * PARTICULAR PURPOSE. See the LICENSE file for the complete terms.
 * Copyleft: covered modifications must retain these license obligations.
 * https://www.gnu.org/licenses/agpl-3.0.html
 */

/** 提供共用的緩動曲線及 Svelte 表面轉場，集中處理減少動態效果的偏好。 */
import type { EasingFunction, TransitionConfig } from "svelte/transition";

export type SurfaceMotionOptions = {
  duration?: number;
  exitDuration?: number;
  offsetY?: number;
  offsetUnit?: "px" | "%";
  startScale?: number;
  easing?: EasingFunction;
};

/** 以二分法反推曲線參數，讓 JavaScript 轉場沿用 CSS 的貝茲緩動。 */
/**
 * 以二分法反推 CSS 貝茲曲線，供 Svelte 轉場使用。
 * @param x1 第一控制點 X。
 * @param y1 第一控制點 Y。
 * @param x2 第二控制點 X。
 * @param y2 第二控制點 Y。
 * @return 輸入及輸出皆為動畫進度的緩動函式。
 */
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

export const modalSurfaceEasing = cubicBezier(0.22, 0.8, 0.28, 1);
export const toastSurfaceEasing = cubicBezier(0.16, 1, 0.3, 1);
export const inlineNoticeSurfaceEasing = cubicBezier(0.23, 1, 0.32, 1);

/**
 * 建立遵守減少動態偏好的表面轉場。
 * @param node 接收入場及退場事件的 DOM 元素。
 * @param options 時間、位移、縮放與緩動選項。
 * @return Svelte TransitionConfig。
 */
export function surfaceMotion(node: Element, options: SurfaceMotionOptions = {}): TransitionConfig {
  const duration = options.duration ?? 220;
  const exitDuration = options.exitDuration ?? duration;
  const offsetY = options.offsetY ?? 8;
  const offsetUnit = options.offsetUnit ?? "px";
  const startScale = options.startScale ?? 0.985;
  const easing = options.easing ?? modalSurfaceEasing;
  const reducedMotion = (): boolean =>
    typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const transitionEasing: EasingFunction = (progress) =>
    reducedMotion() ? progress : easing(progress);
  let transitionPhase: "in" | "out" = "in";
  // 共用可反向播放的轉場，在入場或退場開始時決定對應時間。
  const getDuration = (): number =>
    reducedMotion() ? 120 : transitionPhase === "out" ? exitDuration : duration;
  node.addEventListener("introstart", () => (transitionPhase = "in"));
  node.addEventListener("outrostart", () => (transitionPhase = "out"));

  const transition: TransitionConfig = {
    easing: transitionEasing,
    css: (t, u) => {
      if (reducedMotion())
        return `will-change: transform, opacity; opacity: ${t}; transform: translate3d(0, 0, 0) scale(1);`;
      const translateY = `${u * offsetY}${offsetUnit}`;
      const scale = startScale + (1 - startScale) * t;
      return `will-change: transform, opacity; opacity: ${t}; transform: translate3d(0, ${translateY}, 0) scale(${scale});`;
    },
  };
  Object.defineProperty(transition, "duration", { enumerable: true, get: getDuration });
  return transition;
}
