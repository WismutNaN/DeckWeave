<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { EditorSession } from "../editor/session.svelte";
  let {
    session,
    open = false,
    onclose,
    onexample,
  }: {
    session: EditorSession;
    open: boolean;
    onclose: () => void;
    onexample: () => void;
  } = $props();
  let dialog = $state<HTMLDialogElement>();
  const t = (ru: string, en: string) => session.t(ru, en);
  $effect(() => {
    if (open && !dialog?.open) dialog?.showModal();
    else if (!open && dialog?.open) dialog.close();
  });
  let candidates = $derived(
    (session.snapshot?.installed ?? []).flatMap((p) => {
      const live =
        session.snapshot?.nodes
          .filter((n) => n.packageId === p.package.packageId)
          .map((n) => n.descriptor) ?? [];
      const nodes = [
        ...live,
        ...p.templates.filter((n) => !live.some((l) => l.nodeId === n.nodeId)),
      ];
      return nodes.flatMap((n) =>
        n.devices.map((device) => ({
          package: p.package,
          nodeId: n.nodeId,
          device,
        })),
      );
    }),
  );
</script>

<dialog class="device-picker" bind:this={dialog} {onclose}>
  <div class="dialog-heading">
    <div>
      <h2>{t("Добавить устройство", "Add a device")}</h2>
      <p>
        {t(
          "Подключённые устройства и шаблоны установленных плагинов",
          "Connected devices and templates from installed plugins",
        )}
      </p>
    </div>
    <button
      class="icon-button"
      aria-label={t("Закрыть", "Close")}
      onclick={onclose}><Icon name="close" /></button
    >
  </div>
  <div class="picker-list">
    {#each candidates as candidate}<button
        class="picker-device"
        disabled={session.busy || Boolean(session.pending)}
        onclick={() => {
          session.addDevice(
            candidate.package,
            candidate.nodeId,
            candidate.device,
          );
          onclose();
        }}
        ><span class="picker-icon"><Icon name="device" size={26} /></span><span
          ><strong>{candidate.device.label}</strong><small
            >{candidate.device.elements.length}
            {t("элементов", "controls")} · {candidate.package.version}</small
          ></span
        ><Icon name="plus" /></button
      >{:else}<div class="picker-empty">
        <Icon name="device" size={36} />
        <h3>{t("Пока нет доступных устройств", "No devices available yet")}</h3>
        <p>
          {t(
            "Подключение оборудования обеспечивают плагины. После установки подходящего пакета его устройства появятся здесь.",
            "Device plugins provide hardware connections. Devices appear here after the matching package is installed.",
          )}
        </p>
        <button
          class="subtle-button"
          disabled={session.busy}
          onclick={() => session.run(() => session.refresh())}
          ><Icon name="refresh" size={15} />{t(
            "Обновить список",
            "Refresh list",
          )}</button
        >
      </div>{/each}
  </div>
  {#if !session.devices.length}<div class="example-option">
      <div>
        <strong>{t("Сначала попробовать?", "Try it first?")}</strong>
        <p>
          {t(
            "Учебная панель без подключения к оборудованию.",
            "A practice panel with no hardware connection.",
          )}
        </p>
      </div>
      <button
        class="subtle-button"
        onclick={() => {
          onclose();
          onexample();
        }}
        >{t("Открыть пример", "Open example")}<Icon
          name="arrow"
          size={15}
        /></button
      >
    </div>{/if}
</dialog>
