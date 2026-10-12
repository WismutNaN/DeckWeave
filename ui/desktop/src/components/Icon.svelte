<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  let { name, size = 18 }: { name: string; size?: number } = $props();
  const paths: Record<string, string> = {
    play: "M8 4l12 8-12 8z",
    settings: "M4 7h5 M13 7h7 M4 17h11 M19 17h1 M9 3v8 M15 13v8",
    alert: "M12 3L2 21h20z M12 9v5 M12 18h.01",
    grid: "M3 3h7v7H3z M14 3h7v7h-7z M3 14h7v7H3z M14 14h7v7h-7z",
    plus: "M12 5v14 M5 12h14",
    search: "M21 21l-5-5 M18 10a8 8 0 1 1-16 0 8 8 0 0 1 16 0",
    arrow: "M5 12h14 M13 6l6 6-6 6",
    check: "M5 12l4 4L19 6",
    undo: "M3 10h11a6 6 0 0 1 0 12 M3 10l5-5 M3 10l5 5",
    redo: "M21 10H10a6 6 0 0 0 0 12 M21 10l-5-5 M21 10l-5 5",
    close: "M6 6l12 12 M6 18L18 6",
    sliders: "M4 7h5 M13 7h7 M4 17h11 M19 17h1 M9 3v8 M15 13v8",
    layers: "M12 3l10 6-10 6L2 9z M2 14l10 6 10-6",
    device: "M3 4h18v14H3z M8 22h8 M12 18v4",
    bolt: "M13 2L4 14h7l-1 8 10-13h-7z",
    chevron: "M9 5l7 7-7 7",
    refresh: "M20 7V2l-3 3a9 9 0 1 0 4 8 M20 7h-5",
    fit: "M8 3H3v5 M16 3h5v5 M3 16v5h5 M21 16v5h-5",
    minus: "M5 12h14",
    more: "M5 12h.01 M12 12h.01 M19 12h.01",
    text: "M4 5h16 M12 5v15 M8 20h8",
    palette:
      "M12 3a9 9 0 1 0 0 18h1a3 3 0 0 0 0-6h-1a2 2 0 0 1 0-4h6a3 3 0 0 0 3-3c0-3-5-5-9-5z",
    save: "M5 3h12l4 4v14H3V3z M7 3v6h9V3 M7 21v-7h10v7",
  };
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.6"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"><path d={paths[name] ?? paths.grid} /></svg
>
