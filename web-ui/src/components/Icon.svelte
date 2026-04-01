<script lang="ts">
  // Accessible SVG icon component (audit C1)
  // All icon-only buttons MUST use aria-label on the button, not here.
  let {
    name,
    size = 20,
    class: className = "",
  }: {
    name: string;
    size?: number;
    class?: string;
  } = $props();
</script>

<svg
  class={className}
  width={size}
  height={size}
  fill="none"
  stroke="currentColor"
  viewBox="0 0 24 24"
  stroke-width="2"
  aria-hidden="true"
>
  {#if name === "arrow-down"}
    <path d="M12 4v16m0 0l-6-6m6 6l6-6" />
  {:else if name === "newspaper"}
    <path d="M19 3H5a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2V5a2 2 0 00-2-2zM7 7h10M7 11h4m-4 4h10" />
  {:else if name === "puzzle"}
    <path d="M11 4a2 2 0 114 0v1a1 1 0 001 1h3a2 2 0 012 2v3a1 1 0 01-1 1 2 2 0 100 4 1 1 0 011 1v3a2 2 0 01-2 2h-3a1 1 0 01-1-1 2 2 0 10-4 0 1 1 0 01-1 1H7a2 2 0 01-2-2v-3a1 1 0 011-1 2 2 0 100-4 1 1 0 01-1-1V7a2 2 0 012-2h3a1 1 0 001-1V4z" />
  {:else if name === "clock"}
    <path d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
  {:else if name === "gear"}
    <path d="M10.3 4.3a1 1 0 011.4 0l.3.3a1 1 0 001.1.2 8 8 0 011.5.6 1 1 0 00.9-.3l.3-.3a1 1 0 011.4 0l1 1a1 1 0 010 1.4l-.3.3a1 1 0 00-.2 1.1c.3.5.5 1 .6 1.5a1 1 0 00.8.7h.4a1 1 0 011 1v1.4a1 1 0 01-1 1h-.4a1 1 0 00-.8.7 8 8 0 01-.6 1.5 1 1 0 00.2 1.1l.3.3a1 1 0 010 1.4l-1 1a1 1 0 01-1.4 0l-.3-.3a1 1 0 00-1.1-.2 8 8 0 01-1.5.6 1 1 0 00-.7.8v.4a1 1 0 01-1 1h-1.4a1 1 0 01-1-1v-.4a1 1 0 00-.7-.8 8 8 0 01-1.5-.6 1 1 0 00-1.1.2l-.3.3a1 1 0 01-1.4 0l-1-1a1 1 0 010-1.4l.3-.3a1 1 0 00.2-1.1 8 8 0 01-.6-1.5 1 1 0 00-.8-.7H4a1 1 0 01-1-1v-1.4a1 1 0 011-1h.4a1 1 0 00.8-.7c.1-.5.3-1 .6-1.5a1 1 0 00-.2-1.1l-.3-.3a1 1 0 010-1.4l1-1a1 1 0 011.4 0l.3.3a1 1 0 001.1.2 8 8 0 011.5-.6 1 1 0 00.7-.8V4a1 1 0 011-1h1.4" />
    <circle cx="12" cy="12" r="3" />
  {:else if name === "menu"}
    <path d="M4 6h16M4 12h16M4 18h16" />
  {:else if name === "sun"}
    <path d="M12 3v1m0 16v1m-8-9H3m18 0h-1m-2.6-6.4l-.7.7m-9.4 9.4l-.7.7m12.8 0l-.7-.7M6 6l-.7-.7M16 12a4 4 0 11-8 0 4 4 0 018 0z" />
  {:else if name === "moon"}
    <path d="M20.4 12.1a8 8 0 01-11.5-7.3 8 8 0 1011.5 7.3z" />
  {:else if name === "x"}
    <path d="M6 18L18 6M6 6l12 12" />
  {:else if name === "pause"}
    <path d="M10 9v6m4-6v6" />
  {:else if name === "play"}
    <path d="M8 5.14v14l11-7-11-7z" />
  {:else if name === "trash"}
    <path d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
  {:else if name === "refresh"}
    <path d="M4 4v5h5M20 20v-5h-5" />
    <path d="M20.5 9A9 9 0 005.6 5.6L4 9m16 6l-1.6 3.4A9 9 0 013.5 15" />
  {:else if name === "grip"}
    <circle cx="9" cy="5" r="1" fill="currentColor" />
    <circle cx="15" cy="5" r="1" fill="currentColor" />
    <circle cx="9" cy="12" r="1" fill="currentColor" />
    <circle cx="15" cy="12" r="1" fill="currentColor" />
    <circle cx="9" cy="19" r="1" fill="currentColor" />
    <circle cx="15" cy="19" r="1" fill="currentColor" />
  {:else if name === "chevron-left"}
    <path d="M15 19l-7-7 7-7" />
  {:else if name === "chevron-right"}
    <path d="M9 5l7 7-7 7" />
  {:else if name === "rss"}
    <path d="M4 11a9 9 0 019 9M4 4a16 16 0 0116 16" />
    <circle cx="5" cy="19" r="1.5" fill="currentColor" />
  {:else if name === "plus"}
    <path d="M12 5v14m-7-7h14" />
  {:else if name === "flag"}
    <path d="M4 15s1-1 4-1 5 2 8 2 4-1 4-1V3s-1 1-4 1-5-2-8-2-4 1-4 1zM4 22v-7" />
  {:else if name === "external"}
    <path d="M18 13v6a2 2 0 01-2 2H5a2 2 0 01-2-2V8a2 2 0 012-2h6M15 3h6v6M10 14L21 3" />
  {:else if name === "info"}
    <circle cx="12" cy="12" r="10" />
    <path d="M12 16v-4M12 8h.01" />
  {:else if name === "upload"}
    <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4M17 8l-5-5-5 5M12 3v12" />
  {:else if name === "bolt"}
    <path d="M13 2L3 14h9l-1 8 10-12h-9l1-8z" />
  {/if}
</svg>
