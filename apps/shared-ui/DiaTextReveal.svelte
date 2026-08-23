<script lang="ts">
  export let text = "";
  export let colors: string[] = ["#258DE9", "#389DF6", "#CCE4FF"];
  export let className = "";
  export let revealDuration = 860;
  export let letterDelay = 72;
  export let gradientDuration = 2000;
  export let paused = false;

  $: characters = Array.from(text);
  $: gradient = colors.length > 0 ? colors.join(", ") : "#389df6";
</script>

<span
  class={`dia-text-reveal ${className}`}
  style={`--dia-gradient: ${gradient}; --dia-reveal-duration: ${revealDuration}ms; --dia-letter-delay: ${letterDelay}ms; --dia-gradient-duration: ${gradientDuration}ms;`}
  role="img"
  aria-label={text}
>
  {#each characters as character, index}
    <span
      class:paused
      class="dia-text-reveal__letter"
      style={`--dia-index: ${index};`}
      aria-hidden="true"
    >{character === " " ? "\u00a0" : character}</span>
  {/each}
</span>

<style>
  .dia-text-reveal {
    display: inline-flex;
    align-items: center;
    color: #389df6;
    white-space: pre;
  }

  .dia-text-reveal__letter {
    display: inline-block;
    color: transparent;
    background: linear-gradient(115deg, var(--dia-gradient));
    background-size: 200% 160%;
    background-position: 0% 50%;
    background-clip: text;
    -webkit-background-clip: text;
    filter: blur(9px);
    opacity: 0;
    transform: translate3d(0, 0.42em, 0) scale(0.84);
    backface-visibility: hidden;
    will-change: transform, opacity, filter, background-position;
    animation:
      dia-letter-reveal var(--dia-reveal-duration) cubic-bezier(.16, 1, .3, 1) calc(var(--dia-index) * var(--dia-letter-delay)) forwards,
      dia-gradient-sweep var(--dia-gradient-duration) cubic-bezier(.16, 1, .3, 1) both;
  }

  .dia-text-reveal__letter.paused {
    animation: none;
  }

  @keyframes dia-letter-reveal {
    0% {
      filter: blur(9px);
      opacity: 0;
      transform: translate3d(0, 0.42em, 0) scale(0.84);
    }
    58% {
      filter: blur(0);
      opacity: 1;
      transform: translate3d(0, -0.045em, 0) scale(1.02);
    }
    100% {
      filter: blur(0);
      opacity: 1;
      transform: translate3d(0, 0, 0) scale(1);
    }
  }

  @keyframes dia-gradient-sweep {
    0% { background-position: 0% 50%; }
    68% { background-position: 100% 50%; }
    100% { background-position: 50% 50%; }
  }

  @media (prefers-reduced-motion: reduce) {
    .dia-text-reveal__letter {
      filter: none;
      opacity: 1;
      transform: none;
      background-position: 50% 50%;
      animation: none;
    }
  }
</style>
