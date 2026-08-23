export interface RippleOptions {
  rippleColor?: string;
  durationMs?: number;
}

type RippleElement = HTMLElement & { __bottingRippleCleanup?: () => void };

export function ripple(node: HTMLElement, options: RippleOptions = {}) {
  let currentOptions = options;
  const element = node as RippleElement;

  function applyOptions(next: RippleOptions): void {
    currentOptions = next;
    node.style.setProperty("--ripple-color", next.rippleColor ?? "#ADD8E6");
  }

  function handleClick(event: MouseEvent): void {
    if (node.matches(":disabled") || window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;

    const bounds = node.getBoundingClientRect();
    const size = Math.max(bounds.width, bounds.height) * 2.2;
    const centerX = event.detail === 0 ? bounds.width / 2 : event.clientX - bounds.left;
    const centerY = event.detail === 0 ? bounds.height / 2 : event.clientY - bounds.top;
    const wave = document.createElement("span");
    wave.className = "ripple-wave";
    wave.style.width = `${size}px`;
    wave.style.height = `${size}px`;
    wave.style.left = `${centerX - size / 2}px`;
    wave.style.top = `${centerY - size / 2}px`;
    node.querySelectorAll(".ripple-wave").forEach((wave) => wave.remove());
    node.append(wave);

    const duration = Math.min(160, Math.max(100, currentOptions.durationMs ?? 140));
    window.setTimeout(() => wave.remove(), duration + 40);
  }

  applyOptions(currentOptions);
  node.classList.add("ripple-button");
  node.addEventListener("click", handleClick);
  element.__bottingRippleCleanup = () => {
    node.removeEventListener("click", handleClick);
    node.classList.remove("ripple-button");
    node.style.removeProperty("--ripple-color");
  };

  return {
    update: applyOptions,
    destroy: () => element.__bottingRippleCleanup?.(),
  };
}
