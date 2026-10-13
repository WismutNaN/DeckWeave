// SPDX-License-Identifier: GPL-3.0-or-later
// Internal transfers use pointer capture: WebView2's OLE drag loop can lose a fast drop.
// HTML5 handlers remain an interoperable path for native/synthetic DataTransfer events.
export type TransferDrop = Pick<
  DragEvent,
  | "dataTransfer"
  | "shiftKey"
  | "clientX"
  | "clientY"
  | "preventDefault"
  | "stopPropagation"
>;
type Zone = {
  accept: (data: DataTransfer) => boolean;
  drop: (event: TransferDrop) => void;
};
const zones = new WeakMap<HTMLElement, Zone>();
export function dropZone(node: HTMLElement, value: Zone) {
  node.dataset.editorDrop = "";
  zones.set(node, value);
  return {
    update(next: Zone) {
      zones.set(node, next);
    },
    destroy() {
      zones.delete(node);
      delete node.dataset.editorDrop;
    },
  };
}
export function dragSource(
  node: HTMLElement,
  prepare: (data: DataTransfer) => boolean,
) {
  let active: {
    id: number;
    x: number;
    y: number;
    data: DataTransfer;
    moved: boolean;
  } | null = null;
  let target: HTMLElement | null = null,
    ghost: HTMLElement | null = null,
    suppressClick = false;
  const mark = (next: HTMLElement | null) => {
    if (target === next) return;
    if (target) delete target.dataset.pointerDrop;
    target = next;
    if (target) target.dataset.pointerDrop = "";
  };
  const find = (e: PointerEvent) => {
    let element = document.elementFromPoint(
      e.clientX,
      e.clientY,
    ) as HTMLElement | null;
    while (element) {
      const zone = zones.get(element);
      if (zone && active && zone.accept(active.data)) return element;
      element = element.parentElement;
    }
    return null;
  };
  const reset = () => {
    const previous = active;
    active = null;
    if (previous && node.hasPointerCapture(previous.id))
      node.releasePointerCapture(previous.id);
    mark(null);
    ghost?.remove();
    ghost = null;
    delete node.dataset.pointerDragging;
  };
  const down = (e: PointerEvent) => {
    if (e.button !== 0 || active) return;
    suppressClick = false;
    const data = new DataTransfer();
    if (!prepare(data)) return;
    e.stopPropagation();
    active = {
      id: e.pointerId,
      x: e.clientX,
      y: e.clientY,
      data,
      moved: false,
    };
    node.setPointerCapture(e.pointerId);
  };
  const move = (e: PointerEvent) => {
    if (!active || e.pointerId !== active.id) return;
    if (
      !active.moved &&
      Math.hypot(e.clientX - active.x, e.clientY - active.y) < 5
    )
      return;
    e.preventDefault();
    e.stopPropagation();
    active.moved = true;
    node.dataset.pointerDragging = "";
    if (!ghost) {
      ghost = document.createElement("div");
      ghost.className = "transfer-ghost";
      ghost.textContent = (
        node.getAttribute("aria-label") ||
        node.textContent ||
        ""
      )
        .trim()
        .slice(0, 100);
      document.body.append(ghost);
    }
    ghost.style.left = `${Math.min(e.clientX + 14, window.innerWidth - 230)}px`;
    ghost.style.top = `${Math.min(e.clientY + 14, window.innerHeight - 48)}px`;
    mark(find(e));
  };
  const up = (e: PointerEvent) => {
    if (!active || e.pointerId !== active.id) return;
    const data = active.data,
      moved = active.moved,
      destination = find(e);
    suppressClick = moved;
    reset();
    if (!moved) return;
    e.preventDefault();
    e.stopPropagation();
    if (destination)
      zones.get(destination)?.drop({
        dataTransfer: data,
        shiftKey: e.shiftKey,
        clientX: e.clientX,
        clientY: e.clientY,
        preventDefault() {},
        stopPropagation() {},
      });
  };
  const click = (e: MouseEvent) => {
    if (suppressClick && e.detail !== 0) {
      suppressClick = false;
      e.preventDefault();
      e.stopImmediatePropagation();
    }
  };
  const cancelKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") reset();
  };
  node.addEventListener("pointerdown", down);
  node.addEventListener("pointermove", move);
  node.addEventListener("pointerup", up);
  node.addEventListener("pointercancel", reset);
  node.addEventListener("lostpointercapture", reset);
  node.addEventListener("click", click, true);
  window.addEventListener("blur", reset);
  window.addEventListener("keydown", cancelKey);
  return {
    update(next: typeof prepare) {
      prepare = next;
    },
    destroy() {
      reset();
      node.removeEventListener("pointerdown", down);
      node.removeEventListener("pointermove", move);
      node.removeEventListener("pointerup", up);
      node.removeEventListener("pointercancel", reset);
      node.removeEventListener("lostpointercapture", reset);
      node.removeEventListener("click", click, true);
      window.removeEventListener("blur", reset);
      window.removeEventListener("keydown", cancelKey);
    },
  };
}
