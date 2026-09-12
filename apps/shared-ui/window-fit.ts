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

/** 以同一縮放比例將設計尺寸放入工作區；保留 WebView 可用的最低縮放值。 */
export interface WindowDimensions {
  readonly width: number;
  readonly height: number;
}

export interface FittedWindow {
  readonly scale: number;
  readonly size: WindowDimensions;
}

const minimumWebviewScale = 0.2;

/**
 * 以等比例縮放將設計尺寸放入工作區。
 * @param designSize 固定設計寬高，須為正值。
 * @param workArea 可用工作區尺寸；省略時保留原尺寸。
 * @param edgeMargin 從可用寬高各扣除的邊距。
 * @return 縮放值及尺寸；比例不低於 WebView 的最小值。
 */
export function fitWindowToWorkArea(
  designSize: WindowDimensions,
  workArea?: WindowDimensions,
  edgeMargin = 0,
): FittedWindow {
  if (!workArea) return { scale: 1, size: designSize };

  const margin = Math.max(0, edgeMargin);
  const availableWidth = Math.max(0, workArea.width - margin);
  const availableHeight = Math.max(0, workArea.height - margin);
  const scale = Math.max(
    minimumWebviewScale,
    Math.min(1, availableWidth / designSize.width, availableHeight / designSize.height),
  );

  return {
    scale,
    size: {
      width: designSize.width * scale,
      height: designSize.height * scale,
    },
  };
}
