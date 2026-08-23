import { fitWindowToWorkArea, type WindowDimensions } from "./windowFit";

function smallestPositive(values: Array<number | undefined>): number {
  const positive = values.filter((value): value is number => Number.isFinite(value) && value! > 0);
  return positive.length > 0 ? Math.min(...positive) : 0;
}

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

export function canvasScaleForCurrentViewport(designSize: WindowDimensions): number {
  return fitWindowToWorkArea(designSize, currentViewportSize()).scale;
}

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

export async function waitForViewportLayout(): Promise<void> {
  await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
  await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
}
