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

/** 按目前可見範圍縮放固定畫布，並監聽視窗與 visualViewport 的尺寸變化。 */
import { fitWindowToWorkArea, type WindowDimensions } from "./window-fit";

function smallestPositive(values: Array<number | undefined>): number {
  const positive = values.filter(
    (value): value is number => typeof value === "number" && Number.isFinite(value) && value > 0,
  );
  return positive.length > 0 ? Math.min(...positive) : 0;
}

/**
 * 取內部視窗、文件及 visualViewport 中最小有效尺寸。
 * @return 目前可見的寬度與高度。
 */
export function currentViewportSize(): WindowDimensions {
  const visualViewport = window.visualViewport;
  return {
    width: smallestPositive([
      window.innerWidth,
      document.documentElement.clientWidth,
      visualViewport?.width,
    ]),
    height: smallestPositive([
      window.innerHeight,
      document.documentElement.clientHeight,
      visualViewport?.height,
    ]),
  };
}

/**
 * 計算設計畫布適合目前可見範圍的縮放值。
 * @param designSize 固定畫布的設計尺寸。
 * @return 受最小 WebView 縮放限制的比例。
 */
export function canvasScaleForCurrentViewport(designSize: WindowDimensions): number {
  return fitWindowToWorkArea(designSize, currentViewportSize()).scale;
}

/**
 * 立即同步畫布縮放並監聽視窗尺寸改變。
 * @param designSize 固定畫布的設計尺寸。
 * @param applyScale 接收新縮放值的回呼。
 * @return 解除所有 resize 監聽的清理函式。
 */
export function watchCanvasScale(
  designSize: WindowDimensions,
  applyScale: (scale: number) => void,
): () => void {
  const update = () => applyScale(canvasScaleForCurrentViewport(designSize));
  update();
  window.addEventListener("resize", update);
  window.visualViewport?.addEventListener("resize", update);
  return () => {
    window.removeEventListener("resize", update);
    window.visualViewport?.removeEventListener("resize", update);
  };
}

/** 等待兩次繪製機會，讓原生視窗調整後的 WebView 尺寸先完成更新。 */
/**
 * 等待兩個 animation frame，讓 WebView 尺寸完成更新。
 * @return 版面穩定等待的 Promise。
 */
export async function waitForViewportLayout(): Promise<void> {
  await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
  await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
}
