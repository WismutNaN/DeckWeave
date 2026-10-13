// SPDX-License-Identifier: GPL-3.0-or-later
// Compiled into smoke-test builds only; uses the real WebView IPC and isolated test root.
(() => {
  const invoke = (name, args = {}) =>
    window.__TAURI_INTERNALS__.invoke(name, args);
  const assert = (condition, message) => {
    if (!condition) throw new Error(message);
  };
  const reject = async (work, code) => {
    try {
      await work();
    } catch (error) {
      assert(
        error.code === code && Boolean(error.traceId),
        `Expected ${code}: ${JSON.stringify(error)}`,
      );
      return;
    }
    throw new Error(`Expected rejection: ${code}`);
  };
  async function editor() {
    const initial = await invoke("open_workspace");
    assert(
      initial.draftId === "desktop-main" && initial.version >= 1,
      "Open draft DTO",
    );
    const catalog = await invoke("app_query", {
      query: { type: "editor_snapshot" },
    });
    assert(
      Array.isArray(catalog.installed) && Array.isArray(catalog.nodes),
      "Editor read projection",
    );
    await reject(
      () =>
        invoke("app_query", {
          query: {
            type: "plugin_catalog",
            package: {
              packageId: "org.deckweave.missing",
              version: "1.0.0",
              artifactSha256: "broken",
            },
          },
        }),
      "invalid_command",
    );
    const window = await invoke("mutation_window");
    const document = structuredClone(initial.document);
    document.extensions["org.deckweave.smoke"] = { preserved: true };
    const envelope = {
      requestId: "smoke-save",
      idempotencyKey: "smoke-save",
      dedupEpoch: window.dedupEpoch,
      dryRun: false,
      command: {
        type: "patch_draft",
        draft_id: initial.draftId,
        expected_version: initial.version,
        edit: {
          type: "replace_configuration",
          document,
          package_lock: initial.packageLock,
        },
      },
    };
    const saved = await invoke("app_command", { envelope });
    const replay = await invoke("app_command", { envelope });
    assert(
      saved.draft.version === initial.version + 1 &&
        replay.draft.version === saved.draft.version &&
        replay.receipt.replayed,
      "Mutation replay",
    );
    assert(
      (await invoke("open_workspace")).version === saved.draft.version,
      "Reopen current revision",
    );
    await reject(
      () =>
        invoke("app_command", {
          envelope: {
            ...envelope,
            idempotencyKey: "smoke-stale",
            requestId: "smoke-stale",
          },
        }),
      "revision_conflict",
    );
    const locale = await invoke("locale_get");
    await reject(
      () =>
        invoke("locale_change", {
          expected: locale.revision,
          next: {
            ...locale,
            revision: locale.revision + 1,
            requestedLocale: "!!!",
          },
        }),
      "invalid_command",
    );
    const changed = await invoke("locale_change", {
      expected: locale.revision,
      next: { ...locale, revision: locale.revision + 1, requestedLocale: "ru" },
    });
    assert(
      (await invoke("locale_reconcile")).context.revision ===
        changed.context.revision,
      "Locale reconcile revision",
    );
    const plan = await invoke("app_query", {
      query: {
        type: "plan_apply",
        draft_id: initial.draftId,
        expected_draft_version: saved.draft.version,
        expected_active: catalog.active,
        allow_disruption: false,
      },
    });
    const apply = {
      requestId: "smoke-apply",
      idempotencyKey: "smoke-apply",
      dedupEpoch: window.dedupEpoch,
      dryRun: false,
      command: {
        type: "apply_draft",
        draft_id: initial.draftId,
        expected_draft_version: saved.draft.version,
        expected_active: catalog.active,
        allow_disruption: false,
        plan_hash: plan.planHash,
      },
    };
    const receipt = await invoke("app_command", { envelope: apply });
    const repeated = await invoke("app_command", { envelope: apply });
    assert(
      receipt.operationId === repeated.operationId,
      "Apply receipt identity",
    );
    const operation = await invoke("app_query", {
      query: { type: "operation", operation_id: receipt.operationId },
    });
    assert(
      operation.state === "succeeded" && operation.runtimeHealth === "applied",
      "Empty workspace activated by production coordinator",
    );
    assert(
      (await invoke("app_query", { query: { type: "editor_snapshot" } })).active
        .revisionId === operation.intent.candidate.revisionId,
      "Active revision projection",
    );
  }
  let attempts = 0;
  const check = async () => {
    const detail = document.body?.innerText ?? "No document body";
    if (detail.includes("Host работает") || detail.includes("Host running")) {
      try {
        if (!sessionStorage.getItem("deckweave-smoke-ipc")) {
          await editor();
          sessionStorage.setItem("deckweave-smoke-ipc", "done");
          location.reload();
          return;
        }
        if (!sessionStorage.getItem("deckweave-smoke-ui"))
          sessionStorage.setItem(
            "deckweave-smoke-ui",
            await window.deckweaveSmokeUi(),
          );
        const stress = await window.deckweaveSmokeStress();
        if (stress)
          await invoke("ready", {
            rendered: true,
            detail:
              "editor IPC; CAS; replay; locale; " +
              sessionStorage.getItem("deckweave-smoke-ui") +
              "; " +
              stress,
          });
      } catch (error) {
        await invoke("ready", {
          rendered: false,
          detail: JSON.stringify(error, Object.getOwnPropertyNames(error)),
        });
      }
    } else if (++attempts === 200) {
      await invoke("ready", { rendered: false, detail });
    } else setTimeout(check, 100);
  };
  void check();
})();
