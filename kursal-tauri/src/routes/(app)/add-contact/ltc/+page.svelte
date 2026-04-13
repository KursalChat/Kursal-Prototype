<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { writeFile, readFile } from "@tauri-apps/plugin-fs";
  import { exportLtc, importLtc } from "$lib/api/ltc";
  import { notifications } from "$lib/state/notifications.svelte";
  import { contactsState } from "$lib/state/contacts.svelte";
  import { goto } from "$app/navigation";
  import Button from "$lib/components/Button.svelte";
  import {
    Download,
    Upload,
    ShieldAlert,
    FolderOpen,
    FileCheckCorner,
  } from "lucide-svelte";

  let mode = $state<"export" | "import">("export");
  let exporting = $state(false);
  let importing = $state(false);
  let importError = $state("");
  let dragging = $state(false);

  let unlistenPromises: Array<Promise<() => void>> = [];

  onMount(() => {
    unlistenPromises.push(
      listen<{ paths: string[] }>("tauri://drag-enter", () => {
        if (mode === "import") dragging = true;
      }),
    );
    unlistenPromises.push(
      listen<{ paths: string[] }>("tauri://drag-leave", () => {
        if (mode === "import") dragging = false;
      }),
    );
    unlistenPromises.push(
      listen<{ paths: string[] }>("tauri://drag-drop", async (event) => {
        if (mode === "import") {
          dragging = false;
          if (event.payload.paths && event.payload.paths.length > 0) {
            await handleImportPath(event.payload.paths[0]);
          }
        }
      }),
    );

    return () => {
      unlistenPromises.forEach((p) => p.then((unlisten) => unlisten()));
    };
  });

  async function handleImportPath(path: string) {
    if (!path.endsWith(".kursal")) {
      importError = "Invalid file type. Please select a .kursal file.";
      return;
    }

    importing = true;
    importError = "";
    try {
      const bytesArr = await readFile(path);
      const bytes = Array.from(bytesArr);
      const contact = await importLtc(bytes);
      contactsState.upsert(contact);
      notifications.push("Contact file imported", "success");
      goto(`/chat/${contact.userId}`);
    } catch (e) {
      const errMsg = String(e);
      if (errMsg.includes("expired")) {
        importError =
          "This contact file has expired. Ask your contact to generate a new one.";
      } else {
        importError =
          "Invalid file. Please select a valid .kursal contact file. " + errMsg;
      }
      console.error("Import failed:", e);
    } finally {
      importing = false;
    }
  }

  async function handleExport() {
    exporting = true;
    try {
      const bytes = await exportLtc();
      const path = await save({
        title: "Save contact file",
        defaultPath: "kursal-contact.kursal",
        filters: [
          {
            name: "Kursal data file",
            extensions: ["kursal", "application/octet-stream"],
          },
        ],
      });

      if (!path) {
        notifications.push("Save cancelled", "info");
        return;
      }

      await writeFile(path, new Uint8Array(bytes));
      notifications.push("Contact file ready", "success");
    } catch (e) {
      if (String(e).toLowerCase().includes("cancel")) {
        notifications.push("Save cancelled", "info");
      } else {
        notifications.push("Failed to export file", "error");
      }
      console.error("Export failed:", e);
    } finally {
      exporting = false;
    }
  }

  async function handleImportSelectedFile(file: File) {
    if (!file.name.endsWith(".kursal")) {
      importError = "Invalid file type. Please select a .kursal file.";
      return;
    }

    importing = true;
    importError = "";
    try {
      const buffer = await file.arrayBuffer();
      const bytes = Array.from(new Uint8Array(buffer));
      const contact = await importLtc(bytes);
      contactsState.upsert(contact);
      notifications.push("Contact file imported", "success");
      goto(`/chat/${contact.userId}`);
    } catch (e) {
      const errMsg = String(e);
      if (errMsg.includes("expired")) {
        importError =
          "This contact file has expired. Ask your contact to generate a new one.";
      } else {
        importError =
          "Invalid file. Please select a valid .kursal contact file.";
      }
      console.error("Import failed:", e);
    } finally {
      importing = false;
    }
  }

  async function handleImportFile(event: Event) {
    const input = event.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;

    await handleImportSelectedFile(file);
    input.value = "";
  }

  let fileInput = $state<HTMLInputElement | null>(null);

  async function handleDropZoneClick() {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        pickerMode: "document",
        fileAccessMode: "copy",
        filters: [
          {
            name: "Kursal data file",
            extensions: ["kursal", "application/octet-stream"],
          },
        ],
      });

      if (selected) {
        const selectedPath = Array.isArray(selected) ? selected[0] : selected;
        const path =
          typeof selectedPath === "string"
            ? selectedPath
            : String((selectedPath as { path?: string }).path ?? selectedPath);
        await handleImportPath(path);
        return;
      }
    } catch (err) {
      console.warn("Dialog picker unavailable, falling back to HTML input", err);
    }

    fileInput?.click();
  }

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
    dragging = true;
  }

  function handleDragLeave(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
    dragging = false;
  }

  async function handleDrop(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
    dragging = false;
    const files = e.dataTransfer?.files;
    if (files && files.length > 0) {
      await handleImportSelectedFile(files[0]);
    }
  }

  function handleDropZoneKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      handleDropZoneClick();
    }
  }
</script>

<div class="ltc-flow">
  <div class="mode-toggle" role="tablist" aria-label="Contact file flow">
    <button
      class="toggle-pill"
      class:active={mode === "export"}
      onclick={() => (mode = "export")}
      role="tab"
      aria-selected={mode === "export"}>Export</button
    >
    <button
      class="toggle-pill"
      class:active={mode === "import"}
      onclick={() => (mode = "import")}
      role="tab"
      aria-selected={mode === "import"}>Import</button
    >
  </div>

  {#if mode === "export"}
    <section class="mode-content">
      <div class="heading-row">
        <div>
          <h3>Create contact file</h3>
          <p class="subtle">You will choose where to save it.</p>
        </div>
      </div>

      <p class="explanation">Generate a .kursal file for long term sharing.</p>

      <Button variant="primary" loading={exporting} onclick={handleExport}>
        <Download size={14} />
        Generate and save file
      </Button>

      <div class="warning">
        <ShieldAlert size={16} />
        <div>
          <strong>Keep this file private.</strong>
          <p>Anyone with it can request a secure session until it expires.</p>
        </div>
      </div>
    </section>
  {:else}
    <section class="mode-content">
      <div class="heading-row">
        <div>
          <h3>Import contact file</h3>
          <p class="subtle">Drop a file here or browse for one.</p>
        </div>
      </div>

      <div
        class="drop-zone"
        class:dragging
        ondragover={handleDragOver}
        ondragleave={handleDragLeave}
        ondrop={handleDrop}
        onclick={handleDropZoneClick}
        onkeydown={handleDropZoneKeydown}
        role="button"
        tabindex="0"
        aria-label="Drop file or click to browse"
      >
        {#if importing}
          <Upload size={28} />
        {:else if dragging}
          <FileCheckCorner size={28} />
        {:else}
          <FolderOpen size={28} />
        {/if}
        <p>{dragging ? "Release to import" : "Drop file here"}</p>
        <span>or click to browse</span>
      </div>

      <input
        type="file"
        accept=".kursal"
        onchange={handleImportFile}
        bind:this={fileInput}
        style="display: none"
        disabled={importing}
      />

      {#if importError}
        <div class="error-message">
          {importError}
        </div>
      {/if}
    </section>
  {/if}
</div>

<style>
  .ltc-flow {
    max-width: 760px;
    display: grid;
    gap: 16px;
  }

  .mode-toggle {
    display: flex;
    gap: 6px;
    background: rgba(15, 23, 42, 0.45);
    border-radius: 12px;
    padding: 4px;
    border: 1px solid rgba(148, 163, 184, 0.22);
    width: fit-content;
  }

  .toggle-pill {
    min-width: 84px;
    padding: 8px 12px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    border-radius: 10px;
    transition: all var(--transition);
    font-weight: 700;
    font-size: 12px;
  }

  .toggle-pill.active {
    background: rgba(51, 65, 85, 0.9);
    color: var(--text-primary);
    border-color: rgba(148, 163, 184, 0.35);
  }

  .mode-content {
    display: flex;
    flex-direction: column;
    gap: 16px;
    border: 1px solid rgba(148, 163, 184, 0.22);
    border-radius: 14px;
    background: rgba(15, 23, 42, 0.5);
    backdrop-filter: blur(14px);
    padding: 18px;
  }

  .heading-row {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 14px;
  }

  h3 {
    margin: 0;
    font-size: 20px;
    line-height: 1.2;
  }

  .subtle {
    margin: 4px 0 0;
    font-size: 13px;
    color: var(--text-muted);
  }

  .explanation {
    margin: 0;
    color: var(--text-secondary);
    font-size: 14px;
    line-height: 1.5;
  }

  .warning {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    background: rgba(251, 191, 36, 0.08);
    border: 1px solid rgba(251, 191, 36, 0.28);
    border-radius: 12px;
    padding: 12px;
    font-size: 13px;
    line-height: 1.55;
  }

  .warning strong {
    color: var(--warning);
  }

  .warning p {
    margin: 4px 0 0;
    color: var(--text-secondary);
  }

  .drop-zone {
    border: 1px dashed rgba(148, 163, 184, 0.3);
    border-radius: 14px;
    padding: 28px 16px;
    text-align: center;
    cursor: pointer;
    transition:
      border-color var(--transition),
      background var(--transition),
      transform var(--transition);
    background: rgba(15, 23, 42, 0.35);
    width: 100%;
    color: var(--text-secondary);
  }

  .drop-zone:hover,
  .drop-zone.dragging {
    border-color: rgba(129, 140, 248, 0.45);
    background: rgba(30, 41, 59, 0.55);
  }

  .drop-zone:focus-visible {
    outline: 2px solid rgba(103, 232, 249, 0.7);
    outline-offset: 2px;
  }

  .drop-zone p {
    margin: 0;
    color: var(--text-primary);
    font-weight: 600;
  }

  .drop-zone span {
    display: block;
    margin-top: 4px;
    font-size: 13px;
    color: var(--text-muted);
  }

  .error-message {
    background: rgba(237, 66, 69, 0.1);
    color: var(--danger);
    padding: 12px;
    border-radius: 10px;
    font-size: 13px;
    line-height: 1.5;
    border: 1px solid rgba(237, 66, 69, 0.35);
  }

  :global(.mode-content .button) {
    width: 100%;
  }

  @media (max-width: 740px) {
    .mode-content {
      padding: 16px;
      border-radius: 16px;
    }

    h3 {
      font-size: 19px;
    }

    .toggle-pill {
      min-width: 72px;
      padding-inline: 10px;
    }
  }
</style>
