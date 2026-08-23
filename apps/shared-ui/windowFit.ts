export interface WindowDimensions {
  readonly width: number;
  readonly height: number;
}

export interface FittedWindow {
  readonly scale: number;
  readonly size: WindowDimensions;
}

const minimumWebviewScale = 0.2;

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
