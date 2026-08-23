<script lang="ts">
  export let hexagons: [number, number][] = [];
  export let className = "";

  const columns = 18;
  const rows = 14;
  const radius = 54;
  const hexHeight = Math.sqrt(3) * radius;
  const columnStep = radius * 1.5;

  function points(column: number, row: number): string {
    const centerX = radius + column * columnStep;
    const centerY = row * hexHeight + (column % 2 ? hexHeight / 2 : 0);

    return [0, 60, 120, 180, 240, 300]
      .map((angle) => {
        const radians = (angle * Math.PI) / 180;
        return `${centerX + radius * Math.cos(radians)},${centerY + radius * Math.sin(radians)}`;
      })
      .join(" ");
  }

  const cells = Array.from({ length: columns * rows }, (_, index) => {
    const column = index % columns;
    const row = Math.floor(index / columns);
    return { column, row, points: points(column, row) };
  });

  let selectedKeys = new Set<string>();
  $: selectedKeys = new Set(hexagons.map(([column, row]) => `${column}:${row}`));
</script>

<svg
  class={`hexagon-pattern ${className}`.trim()}
  viewBox="0 0 720 600"
  preserveAspectRatio="xMidYMid slice"
  aria-hidden="true"
>
  {#each cells as cell}
    <polygon
      points={cell.points}
      class:highlighted={selectedKeys.has(`${cell.column + 1}:${cell.row + 1}`)}
    />
  {/each}
</svg>

<style>
  .hexagon-pattern {
    position: absolute;
    inset: 0;
    z-index: 0;
    width: 100%;
    height: 100%;
    overflow: visible;
    color: var(--identity-pattern-color, #b9c8d7);
    opacity: var(--identity-pattern-opacity, .68);
    /* 保持圖案覆蓋完整卡片，避免傾斜後右上角被父層裁走。 */
    transform: none;
    transform-origin: center;
    -webkit-mask-image: radial-gradient(420px circle at center, #fff, transparent);
    mask-image: radial-gradient(420px circle at center, #fff, transparent);
    pointer-events: none;
  }

  polygon {
    fill: transparent;
    stroke: currentColor;
    stroke-width: 1.35;
    stroke-opacity: .34;
    vector-effect: non-scaling-stroke;
  }

  polygon.highlighted {
    fill: var(--identity-highlight-color, currentColor);
    fill-opacity: var(--identity-highlight-opacity, .2);
    stroke-opacity: .52;
  }

  @media (prefers-reduced-motion: reduce) {
    .hexagon-pattern { transform: none; }
  }
</style>
