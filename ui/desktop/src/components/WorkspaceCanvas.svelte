<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import { dragSource, dropZone } from "../editor/pointer-drag";
  import LiveToolbar from "./LiveToolbar.svelte";
  import type { LiveControls } from "../editor/live.svelte";
  import { tick } from "svelte";
  import {
    assignmentIndex,
    canPan,
    deviceSize,
    visibleControls,
  } from "../editor/viewport";
  import Icon from "./Icon.svelte";
  import { controlBackground, geometry, neighbor } from "../editor/appearance";
  import { equal, owner } from "../editor/bindings";
  import {
    hasActionDrag,
    readSelectionDrag,
    writeSelectionDrag,
  } from "../editor/drag";
  import { dropOnControl } from "../editor/drops";
  import type { EditorSession } from "../editor/session.svelte";
  import type { EditorDevice } from "../editor/types";
  let {
    session,
    live,
    onadd,
  }: { session: EditorSession; live: LiveControls; onadd: () => void } =
    $props();
  let zoom = $state(1),
    pan = $state({ x: 64, y: 110 }),
    space = $state(false),
    area = $state<HTMLElement>(),
    width = $state(900),
    height = $state(700);
  let dropTarget = $state("");
  let drag = $state<{
    id: string;
    startX: number;
    startY: number;
    x: number;
    y: number;
  } | null>(null);
  const t = (ru: string, en: string) => session.t(ru, en);
  let assignments = $derived(assignmentIndex(session.document));
  const position = (device: EditorDevice) =>
    drag?.id === device.id ? { x: drag.x, y: drag.y } : device;
  const visible = (device: EditorDevice) => {
    const p = position(device),
      g = geometry(device.descriptor);
    return (
      (p.x + Math.min(740, Math.max(340, g.width * 88 + 36))) * zoom + pan.x >
        0 &&
      p.x * zoom + pan.x < width &&
      (p.y + g.height * 88 + 100) * zoom + pan.y > 0 &&
      p.y * zoom + pan.y < height
    );
  };
  function start(e: PointerEvent, id = "") {
    const target = e.target as HTMLElement;
    if (
      !id &&
      !canPan(
        e.button,
        space,
        Boolean(target.closest("button, input, select, textarea, a")),
        Boolean(target.closest(".device-object")),
      )
    )
      return;
    if (id && e.button !== 0) return;
    e.stopPropagation();
    e.preventDefault();
    const device = session.devices.find((d) => d.id === id);
    drag = {
      id,
      startX: e.clientX,
      startY: e.clientY,
      x: device?.x ?? pan.x,
      y: device?.y ?? pan.y,
    };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }
  function move(e: PointerEvent) {
    if (!drag) return;
    const scale = drag.id ? zoom : 1;
    const dx = (e.clientX - drag.startX) / scale,
      dy = (e.clientY - drag.startY) / scale;
    drag = {
      ...drag,
      startX: e.clientX,
      startY: e.clientY,
      x: Math.max(-10000, Math.min(10000, drag.x + dx)),
      y: Math.max(-10000, Math.min(10000, drag.y + dy)),
    };
    if (!drag.id) pan = { x: drag.x, y: drag.y };
  }
  function stop() {
    if (drag?.id) {
      const value = drag;
      session.select(
        value.id,
        session.selectedDevice === value.id ? session.selectedControl : "",
      );
      session.updateDevice((d) => {
        d.x = value.x;
        d.y = value.y;
      });
    }
    drag = null;
  }
  function fit() {
    const d = session.device ?? session.devices[0];
    if (d) {
      const g = geometry(d.descriptor),
        w = Math.min(740, Math.max(340, g.width * 88 + 36)),
        h = ((w - 36) * g.height) / g.width + 155;
      zoom = Math.max(0.35, Math.min(1, (width - 48) / w, (height - 135) / h));
      pan = {
        x: (width - w * zoom) / 2 - d.x * zoom,
        y: 70 + Math.max(0, (height - 135 - h * zoom) / 2) - d.y * zoom,
      };
    } else {
      zoom = 1;
      pan = { x: 64, y: 110 };
    }
  }
  let fitted = $state("");
  $effect(() => {
    const key =
      session.devices.map((d) => d.id).join("|") + ":" + width + ":" + height;
    if (key !== fitted) {
      fitted = key;
      fit();
    }
  });
  function wheel(e: WheelEvent) {
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      const next = Math.max(0.35, Math.min(1.8, zoom - e.deltaY * 0.001));
      const bounds = area?.getBoundingClientRect();
      if (bounds) {
        const x = e.clientX - bounds.left,
          y = e.clientY - bounds.top;
        pan = {
          x: x - ((x - pan.x) * next) / zoom,
          y: y - ((y - pan.y) * next) / zoom,
        };
      }
      zoom = next;
    } else {
      pan = { x: pan.x - e.deltaX, y: pan.y - e.deltaY };
    }
  }
  async function navigate(
    e: KeyboardEvent,
    device: EditorDevice,
    index: number,
  ) {
    if (["ArrowRight", "ArrowLeft", "ArrowDown", "ArrowUp"].includes(e.key)) {
      e.preventDefault();
      const next = neighbor(device.descriptor, index, e.key);
      const element = device.descriptor.elements[next];
      session.select(device.id, element.elementId);
      const geo = geometry(device.descriptor),
        scale = deviceSize(device, geo).scale,
        b = geo.controls[element.elementId];
      const x = (device.x + 18 + (b.x + b.width / 2) * scale) * zoom + pan.x;
      const y = (device.y + 70 + (b.y + b.height / 2) * scale) * zoom + pan.y;
      pan = {
        x: pan.x + (x < 40 ? 40 - x : x > width - 40 ? width - 40 - x : 0),
        y: pan.y + (y < 85 ? 85 - y : y > height - 65 ? height - 65 - y : 0),
      };
      await tick();
      area
        ?.querySelector<HTMLButtonElement>(
          `[data-control-id="${CSS.escape(element.elementId)}"][data-device-id="${CSS.escape(device.id)}"]`,
        )
        ?.focus();
    }
  }
  const dropKey = (deviceId: string, elementId: string) =>
    `${deviceId}\u0000${elementId}`;
  function dragOver(e: DragEvent, device: EditorDevice, id: string) {
    if (!hasActionDrag(e.dataTransfer)) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "copy";
    dropTarget = dropKey(device.id, id);
  }
  function dragLeave(e: DragEvent, device: EditorDevice, id: string) {
    const current = e.currentTarget as HTMLElement;
    if (e.relatedTarget instanceof Node && current.contains(e.relatedTarget)) {
      return;
    }
    if (dropTarget === dropKey(device.id, id)) dropTarget = "";
  }
  function drop(e: DragEvent, device: EditorDevice, id: string) {
    dropTarget = "";
    dropOnControl(session, e, device.id, id);
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (
      e.code === "Space" &&
      !["INPUT", "TEXTAREA", "SELECT", "BUTTON", "A"].includes(
        (e.target as HTMLElement).tagName,
      )
    ) {
      space = true;
      e.preventDefault();
    }
  }}
  onkeyup={(e) => {
    if (e.code === "Space") space = false;
  }}
  onblur={() => {
    space = false;
    drag = null;
    dropTarget = "";
  }}
  ondragend={() => (dropTarget = "")}
  ondrop={() => (dropTarget = "")}
/>
<section
  use:dropZone={{
    accept: (data) => readSelectionDrag(data)?.kind === "device",
    drop: (e) => {
      const value = readSelectionDrag(e.dataTransfer);
      if (!value) return;
      const bounds = area!.getBoundingClientRect();
      session.select(value.deviceId);
      session.updateDevice((d) => {
        d.x = (e.clientX - bounds.left - pan.x) / zoom;
        d.y = (e.clientY - bounds.top - pan.y) / zoom;
      });
    },
  }}
  class="workspace-stage"
  aria-label={t("Рабочее пространство устройств", "Device workspace")}
  bind:this={area}
  bind:clientWidth={width}
  bind:clientHeight={height}
  onwheel={wheel}
  ondragover={(e) => {
    if (hasActionDrag(e.dataTransfer)) e.preventDefault();
  }}
  ondrop={(e) => {
    const value = readSelectionDrag(e.dataTransfer);
    if (value?.kind !== "device") return;
    e.preventDefault();
    const bounds = area!.getBoundingClientRect();
    session.select(value.deviceId);
    session.updateDevice((d) => {
      d.x = (e.clientX - bounds.left - pan.x) / zoom;
      d.y = (e.clientY - bounds.top - pan.y) / zoom;
    });
  }}
  onpointerdown={(e) => start(e)}
  onpointermove={move}
  onpointerup={stop}
  onpointercancel={() => {
    drag = null;
  }}
  style:cursor={space ? "grab" : undefined}
>
  <div class="canvas-topline">
    <div class="breadcrumb">
      {t("Рабочее пространство", "Workspace")}
      <Icon name="chevron" size={13} /> <strong>{t("Основное", "Main")}</strong>
    </div>
    <button class="subtle-button" onclick={onadd}
      ><Icon name="plus" size={15} />{t("Устройство", "Device")}</button
    >
  </div>
  <LiveToolbar {live} />
  {#if session.devices.length === 0}<div class="workspace-empty">
      <div class="empty-device"><Icon name="grid" size={38} /></div>
      <h1>{t("Ваше рабочее место", "Your workspace")}</h1>
      <p>
        {t(
          "Добавьте устройство и выберите кнопку, которую хотите настроить.",
          "Add a device and select a control to configure.",
        )}
      </p>
      <button class="primary" onclick={onadd}
        ><Icon name="plus" />{t("Добавить устройство", "Add a device")}</button
      >
    </div>{:else}
    <div
      class="canvas-world"
      style:transform={`translate(${pan.x}px, ${pan.y}px) scale(${zoom})`}
    >
      {#each session.devices.filter(visible) as device (device.id)}{@const geo =
          geometry(device.descriptor)}{@const p = position(device)}
        <article
          class="device-object"
          class:device-selected={session.selectedDevice === device.id}
          style:left={`${p.x}px`}
          style:top={`${p.y}px`}
          style:width={`${Math.min(740, Math.max(340, geo.width * 88 + 36))}px`}
        >
          <button
            class="device-heading"
            onpointerdown={(e) => start(e, device.id)}
            onpointermove={move}
            onpointerup={stop}
            onclick={() => session.select(device.id)}
            ><span class="device-symbol"><Icon name="device" /></span><span
              ><strong>{device.descriptor.label}</strong><small
                >{device.descriptor.kind === "virtual"
                  ? t("Виртуальное устройство", "Virtual device")
                  : t("Устройство", "Device")} · {device.descriptor.elements
                  .length}
                {t("элементов", "controls")}</small
              ></span
            ><span class="device-state"
              >{session.snapshot?.nodes.some(
                (n) =>
                  n.packageId === device.package.packageId &&
                  n.descriptor.nodeId === device.nodeId &&
                  n.descriptor.devices.some(
                    (d) =>
                      d.identity === device.descriptor.identity &&
                      d.providerId === device.descriptor.providerId,
                  ),
              )
                ? t("Подключено", "Connected")
                : session.preview
                  ? t("Учебный пример", "Example")
                  : t("Не подключено", "Offline")}</span
            ></button
          >
          <div class="device-body">
            <div
              class="device-keys"
              style:aspect-ratio={`${geo.width} / ${geo.height}`}
            >
              {#each visibleControls({ ...device, ...p }, { width, height, zoom, pan }, session.selectedDevice === device.id ? session.selectedControl : "", geo) as { element, index } (element.elementId)}{@const bounds =
                  geo.controls[element.elementId]}{@const assigned =
                  assignments.get(owner(device.id, element.elementId))}
                {@const custom = device.controls[element.elementId]}
                <button
                  class="device-key"
                  draggable={false}
                  use:dragSource={(data) => {
                    if (live.enabled || session.editLocked) return false;
                    writeSelectionDrag(data, {
                      kind: "control",
                      deviceId: device.id,
                      controlId: element.elementId,
                    });
                    return true;
                  }}
                  use:dropZone={{
                    accept: (data) =>
                      !live.enabled &&
                      hasActionDrag(data) &&
                      readSelectionDrag(data)?.kind !== "device" &&
                      readSelectionDrag(data)?.kind !== "step",
                    drop: (e) =>
                      dropOnControl(session, e, device.id, element.elementId),
                  }}
                  disabled={live.enabled &&
                    (!live.ready ||
                      live.pending ||
                      device.descriptor.kind !== "virtual")}
                  title={t(
                    "Перетащите для копирования; Shift — перенести",
                    "Drag to copy; Shift to move",
                  )}
                  ondragstart={(e) => {
                    if (e.dataTransfer)
                      writeSelectionDrag(e.dataTransfer, {
                        kind: "control",
                        deviceId: device.id,
                        controlId: element.elementId,
                      });
                  }}
                  data-control-id={element.elementId}
                  data-device-id={device.id}
                  class:drop-target={dropTarget ===
                    dropKey(device.id, element.elementId)}
                  class:selected={session.selectedDevice === device.id &&
                    session.selectedControl === element.elementId}
                  class:assigned={(assigned?.count ?? 0) > 0}
                  aria-pressed={session.selectedDevice === device.id &&
                    session.selectedControl === element.elementId}
                  aria-label={`${device.descriptor.label}: ${custom?.label || element.label || element.elementId}`}
                  style:left={`${(bounds.x / geo.width) * 100}%`}
                  style:top={`${(bounds.y / geo.height) * 100}%`}
                  style:width={`${(bounds.width / geo.width) * 100}%`}
                  style:height={`${(bounds.height / geo.height) * 100}%`}
                  onclick={() => {
                    if (live.enabled)
                      void live.click(device, element.elementId);
                    else session.select(device.id, element.elementId);
                  }}
                  onkeydown={(e) => navigate(e, device, index)}
                  ondragenter={(e) => dragOver(e, device, element.elementId)}
                  ondragover={(e) => dragOver(e, device, element.elementId)}
                  ondragleave={(e) => dragLeave(e, device, element.elementId)}
                  ondrop={(e) => drop(e, device, element.elementId)}
                >
                  <span
                    class="key-face"
                    style:background={controlBackground(
                      device,
                      element.elementId,
                      geo,
                    )}
                    style:color={custom?.foreground ?? "#ffffff"}
                    style:filter={`brightness(${Math.max(10, Math.min(100, custom?.background?.brightness ?? device.appearance.brightness))}%)`}
                    ><span class="key-number">{index + 1}</span
                    >{#if assigned?.count ?? 0}<Icon
                        name="bolt"
                        size={21}
                      />{:else}<span class="key-add">+</span>{/if}<span
                      class="key-label"
                      >{(live.enabled
                        ? live.text(device, element.elementId)
                        : null) ||
                        custom?.label ||
                        (assigned?.first
                          ? (session.actions.find((a) =>
                              equal(a.endpoint, assigned?.first.kind.endpoint),
                            )?.title ?? element.label)
                          : "")}</span
                    >{#if custom?.background}<span
                        class="override-mark"
                        title={t("Собственный фон", "Custom background")}
                        >◆</span
                      >{/if}{#if (assigned?.count ?? 0) > 1}<span
                        class="step-count">{assigned?.count ?? 0}</span
                      >{/if}</span
                  >
                </button>{/each}
            </div>
            <div class="device-edge">
              <span>DECKWEAVE</span><span class="edge-indicator"></span>
            </div>
          </div>
          <div class="device-caption">
            {geo.measured
              ? t("Расположение задано плагином", "Layout provided by plugin")
              : t(
                  "Список элементов · положение неизвестно",
                  "Control list · physical positions unknown",
                )}<button
              class="text-button"
              onclick={() => session.select(device.id)}
              >{t("Общие настройки", "Device settings")}
              <Icon name="sliders" size={13} /></button
            >
          </div>
        </article>{/each}
    </div>{/if}
  <div class="canvas-bottom">
    <span class="canvas-hint"
      >{t(
        "Перетаскивайте пустое поле для перемещения",
        "Drag the empty canvas to pan",
      )}</span
    >
    <div class="zoom-controls">
      <button
        class="icon-button"
        aria-label={t("Уменьшить", "Zoom out")}
        onclick={() => (zoom = Math.max(0.35, zoom - 0.1))}
        ><Icon name="minus" size={15} /></button
      ><output>{Math.round(zoom * 100)}%</output><button
        class="icon-button"
        aria-label={t("Увеличить", "Zoom in")}
        onclick={() => (zoom = Math.min(1.8, zoom + 0.1))}
        ><Icon name="plus" size={15} /></button
      ><span></span><button
        class="icon-button"
        aria-label={t("Показать устройство целиком", "Fit device")}
        onclick={fit}><Icon name="fit" size={16} /></button
      >
    </div>
  </div>
</section>
