<script lang="ts">
  // Sweepr mark, direction F "La grille rangée". Source: design/icon/sweepr.svg.
  let { size = 28 }: { size?: number } = $props();
  const id = $props.id();
</script>

<svg width={size} height={size} viewBox="0 0 100 100" aria-hidden="true" class="shrink-0">
  <defs>
    <linearGradient id="{id}-bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#2e2a78" />
      <stop offset="1" stop-color="#5b50e8" />
    </linearGradient>
  </defs>
  <rect width="100" height="100" rx="22.5" fill="url(#{id}-bg)" />
  <g fill="#fff">
    <rect x="20.5" y="20.5" width="11" height="11" rx="3" />
    <rect x="20.5" y="36.5" width="11" height="11" rx="3" />
    <rect x="36.5" y="36.5" width="11" height="11" rx="3" />
    <rect x="20.5" y="52.5" width="11" height="11" rx="3" />
    <rect x="36.5" y="52.5" width="11" height="11" rx="3" />
    <rect x="52.5" y="52.5" width="11" height="11" rx="3" />
    <rect x="20.5" y="68.5" width="11" height="11" rx="3" />
    <rect x="36.5" y="68.5" width="11" height="11" rx="3" />
    <rect x="52.5" y="68.5" width="11" height="11" rx="3" />
    <rect x="68.5" y="68.5" width="11" height="11" rx="3" />
  </g>
  <g fill="none" stroke="#fff" stroke-width="2" stroke-opacity="0.4">
    <rect x="37.5" y="21.5" width="9" height="9" rx="2.5" />
    <rect x="53.5" y="21.5" width="9" height="9" rx="2.5" />
    <rect x="69.5" y="21.5" width="9" height="9" rx="2.5" />
    <rect x="53.5" y="37.5" width="9" height="9" rx="2.5" />
    <rect x="69.5" y="37.5" width="9" height="9" rx="2.5" />
    <rect x="69.5" y="53.5" width="9" height="9" rx="2.5" />
  </g>
</svg>
