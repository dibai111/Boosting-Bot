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

/** 協調原生視窗、WebView 縮放及檔案拖放；版面切換會依序完成。 */
import { invoke } from "@tauri-apps/api/core";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { currentMonitor, getCurrentWindow, type DragDropEvent } from "@tauri-apps/api/window";
import { fitWindowToWorkArea } from "../../../../shared-ui/window-fit";
import {
  canvasScaleForCurrentViewport,
  waitForViewportLayout,
} from "../../../../shared-ui/viewport-fit";

const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export type UserWindowMode = "entry" | "app";

const entryWindowSize = { width: 514, height: 350 } as const;
const appWindowSize = { width: 1180, height: 760 } as const;
const windowEdgeMargin = 24;

interface PhysicalWindowBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface WindowTarget {
  bounds: PhysicalWindowBounds | null;
  size: LogicalSize;
}

function setEntrySurfaceScale(): void {
  const scaleX = entryWindowSize.width / appWindowSize.width;
  const scaleY = entryWindowSize.height / appWindowSize.height;
  const rootStyle = document.documentElement.style;
  rootStyle.setProperty("--app-entry-start-scale-x", String(scaleX));
  rootStyle.setProperty("--app-entry-start-scale-y", String(scaleY));
  rootStyle.setProperty("--app-entry-start-radius-x", `${18 / scaleX}px`);
  rootStyle.setProperty("--app-entry-start-radius-y", `${18 / scaleY}px`);
}

function fitCanvasToViewport(): void {
  const isEntry = document.documentElement.dataset.userWindow === "entry";
  const designSize = isEntry ? entryWindowSize : appWindowSize;
  const property = isEntry ? "--entry-canvas-scale" : "--user-canvas-scale";
  document.documentElement.style.setProperty(
    property,
    String(canvasScaleForCurrentViewport(designSize)),
  );
}

async function resetWebviewZoom(): Promise<void> {
  try {
    await getCurrentWebview().setZoom(1);
  } catch (error) {
    console.warn(
      "Unable to reset the user WebView zoom; fitting the canvas to the viewport.",
      error,
    );
  }
}

/**
 * 依可見範圍同步畫布縮放，並訂閱尺寸變化。
 * @return 解除 resize 監聽的清理函式。
 */
export function watchUserCanvas(): () => void {
  const update = () => fitCanvasToViewport();
  update();
  window.addEventListener("resize", update);
  window.visualViewport?.addEventListener("resize", update);
  return () => {
    window.removeEventListener("resize", update);
    window.visualViewport?.removeEventListener("resize", update);
  };
}

async function fitWindowTarget(
  designSize: typeof entryWindowSize | typeof appWindowSize,
): Promise<WindowTarget> {
  const monitor = await currentMonitor();
  const workArea = monitor?.workArea.size.toLogical(monitor.scaleFactor);
  const fitted = fitWindowToWorkArea(designSize, workArea, windowEdgeMargin);
  const size = new LogicalSize(fitted.size.width, fitted.size.height);
  const physicalSize = monitor ? size.toPhysical(monitor.scaleFactor) : null;
  const width = physicalSize ? Math.round(physicalSize.width) : 0;
  const height = physicalSize ? Math.round(physicalSize.height) : 0;
  return {
    bounds: monitor
      ? {
          x: monitor.workArea.position.x + Math.round((monitor.workArea.size.width - width) / 2),
          y: monitor.workArea.position.y + Math.round((monitor.workArea.size.height - height) / 2),
          width,
          height,
        }
      : null,
    size,
  };
}

// 將原生版面操作串接，避免相鄰切換互相覆蓋尺寸和可見狀態。
let windowModeSequence = Promise.resolve();
/**
 * 依序調整視窗、WebView 與畫布縮放，避免相鄰切換覆寫。
 * @param mode entry 或 app 視窗模式。
 * @return 版面切換完成的 Promise。
 */
export async function setUserWindowMode(mode: UserWindowMode): Promise<void> {
  if (!isTauri) {
    document.documentElement.dataset.userWindow = mode === "app" ? "main" : "entry";
    fitCanvasToViewport();
    return;
  }

  windowModeSequence = windowModeSequence
    .catch(() => undefined)
    .then(async () => {
      const target = await fitWindowTarget(mode === "app" ? appWindowSize : entryWindowSize);
      const appWindow = getCurrentWindow();
      await resetWebviewZoom();
      if (mode === "app") {
        setEntrySurfaceScale();
      }
      if (mode === "entry") {
        await appWindow.setSize(target.size);
        await appWindow.center();
      } else if (target.bounds) {
        await invoke("set_main_window_layout", {
          x: target.bounds.x,
          y: target.bounds.y,
          width: target.bounds.width,
          height: target.bounds.height,
        });
      } else {
        await appWindow.setSize(target.size);
        await appWindow.center();
      }
      await waitForViewportLayout();
      document.documentElement.dataset.userWindow = mode === "app" ? "main" : "entry";
      fitCanvasToViewport();
      await appWindow.unminimize();
      await appWindow.show();
    });
  await windowModeSequence;
}

/**
 * 最小化目前 Tauri 視窗。
 * @return 完成的 Promise；瀏覽器環境直接返回。
 */
export async function minimizeWindow(): Promise<void> {
  if (isTauri) await getCurrentWindow().minimize();
}

/**
 * 提出目前視窗的關閉要求，由後端負責停止工作階段。
 * @return 提出要求的 Promise。
 */
export async function closeWindow(): Promise<void> {
  if (isTauri) await getCurrentWindow().close();
}

/**
 * 訂閱 Tauri 原生拖放事件。
 * @param handler 拖放階段、位置及路徑的處理回呼。
 * @return 解除監聽函式的 Promise。
 */
export async function onFileDrop(
  handler: (event: DragDropEvent) => void | Promise<void>,
): Promise<() => void> {
  if (!isTauri) return () => undefined;
  return getCurrentWindow().onDragDropEvent(({ payload }) => handler(payload));
}

export type FileDropEvent = DragDropEvent;
