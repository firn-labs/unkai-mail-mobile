<script lang="ts">
  export let size: number | string = 20;
  let className = '';
  export { className as class };
</script>

<!-- The crescent for "Dark" in Settings → Appearance, drawn to sit beside
     `Sun`: same stroke weight and caps, and a body of the same visual
     size (the sun's disc plus rays spans ~18px; the crescent spans ~16). -->
<svg
  xmlns="http://www.w3.org/2000/svg"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.6"
  stroke-linecap="round"
  stroke-linejoin="round"
  class={className}
  aria-hidden="true"
>
  <path d="M19.5 14.6A7.8 7.8 0 0 1 9.4 4.5a7.8 7.8 0 1 0 10.1 10.1z"/>
</svg>
