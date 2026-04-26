<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { listen } from "@tauri-apps/api/event";
  import { goto } from "$app/navigation";
  import { contactsState } from "$lib/state/contacts.svelte";
  import { messagesState } from "$lib/state/messages.svelte";
  import { nearbyState } from "$lib/state/nearby.svelte";
  import { typingState } from "$lib/state/typing.svelte";
  import { notifications } from "$lib/state/notifications.svelte";
  import { appearanceState } from "$lib/state/appearance.svelte";
  import { prefsState } from "$lib/state/prefs.svelte";
  import { settingsState } from "$lib/state/settings.svelte";
  import { notifyMessage, getPermission } from "$lib/api/system-notify";
  import { frontendReady } from "$lib/api/identity";
  import { OS } from "$lib/api/window";
  import { finalizeDeferredReceiveTarget } from "$lib/utils/file-transfer-paths";
  import { acceptFileOffer } from "$lib/api/messages";
  import ToastContainer from "$lib/components/ToastContainer.svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import type {
    MessageReceivedPayload,
    ConnectionChangedPayload,
    NearbyRequestPayload,
    ContactResponse,
    MessageEditedPayload,
    MessageDeletedPayload,
    ReactionChangedPayload,
    FileOfferedPayload,
    FileTransferProgressPayload,
    FileReceivedPayload,
    TypingIndicatorPayload,
    BackendSignalPayload,
  } from "$lib/types";

  let { children } = $props();
  let backgroundUnread = 0;
  let baseTitle = "Kursal";

  function refreshTitle() {
    document.title =
      backgroundUnread > 0 ? `(${backgroundUnread}) ${baseTitle}` : baseTitle;
  }

  onMount(() => {
    void frontendReady();
    if (OS === "macos") document.documentElement.classList.add("mac");
    appearanceState.init();
    prefsState.init();
    void settingsState.load();
    void getPermission();
    const unlistenPromises: Array<Promise<() => void>> = [];
    baseTitle = document.title || "Kursal";

    const handleVisibilityChange = () => {
      if (!document.hidden && backgroundUnread > 0) {
        backgroundUnread = 0;
        refreshTitle();
      }
    };
    document.addEventListener("visibilitychange", handleVisibilityChange);

    const onboarded = localStorage.getItem("kursal_onboarded");
    if (onboarded !== "done" && $page.url.pathname !== "/onboarding") {
      goto("/onboarding", { replaceState: true });
    }

    // Initial data load
    contactsState.load().then(() => {
      // Restore last viewed chat if it exists
      const lastChatId = localStorage.getItem("kursal_last_chat");
      if (lastChatId && contactsState.getById(lastChatId)) {
        const currentPath = $page.url.pathname;
        if (currentPath === "/chat") {
          goto("/chat/" + lastChatId, { replaceState: true });
        }
      }
    });

    // Listen to backend_signal event
    unlistenPromises.push(
      listen<BackendSignalPayload>("backend_signal", (event) => {
        const signal = event.payload.signal;
        const payload = event.payload.payload;

        if (signal == "open_settings") {
          goto("/settings");
        } else if (signal == "open_otp") {
          goto(`/add-contact/otp?receive=${encodeURIComponent(payload)}`);
        } else if (signal == "handle_incoming_error") {
          notifications.push(payload || "Incoming message error", "error");
        }
      }),
    );

    // Listen to message_received event
    unlistenPromises.push(
      listen<MessageReceivedPayload>("message_received", (event) => {
        const payload = event.payload;
        payload.timestamp = payload.timestamp * 1000; // Rust gives seconds, UI expects ms
        messagesState.append(payload);
        typingState.clear(payload.contactId);
        messagesState.setFirstUnread(payload.contactId, payload.id);
        if (document.hidden) {
          backgroundUnread += 1;
          refreshTitle();
        }
        const onThisChat =
          !document.hidden &&
          $page.url.pathname === `/chat/${payload.contactId}`;
        if (!onThisChat) {
          const name =
            contactsState.getById(payload.contactId)?.displayName ?? "Someone";
          void notifyMessage({ senderName: name, body: payload.content });
        }
      }),
    );

    // Listen to connection_changed event
    unlistenPromises.push(
      listen<ConnectionChangedPayload>("connection_changed", (event) => {
        contactsState.setConnectionStatus(
          event.payload.contactId,
          event.payload.status,
        );
      }),
    );

    // Listen to contact_added event
    unlistenPromises.push(
      listen<ContactResponse>("contact_added", (event) => {
        contactsState.upsert(event.payload);
        notifications.push("Contact added!", "success");
        goto("/chat/" + event.payload.userId);
      }),
    );

    // Listen to delivery_confirmed event
    unlistenPromises.push(
      listen<{ messageId: string; contactId: string }>(
        "delivery_confirmed",
        (event) => {
          messagesState.updateStatus(
            event.payload.messageId,
            event.payload.contactId,
            "delivered",
          );
        },
      ),
    );

    // Listen to message_edited event
    unlistenPromises.push(
      listen<MessageEditedPayload>("message_edited", (event) => {
        messagesState.updateContent(
          event.payload.messageId,
          event.payload.contactId,
          event.payload.newContent,
        );
      }),
    );

    // Listen to message_deleted event
    unlistenPromises.push(
      listen<MessageDeletedPayload>("message_deleted", (event) => {
        messagesState.markDeleted(
          event.payload.messageId,
          event.payload.contactId,
        );
      }),
    );

    // Listen to reaction_added event
    unlistenPromises.push(
      listen<ReactionChangedPayload>("reaction_added", (event) => {
        messagesState.addReaction(
          event.payload.messageId,
          event.payload.contactId,
          event.payload.emoji,
          event.payload.contactId,
        );
      }),
    );

    // Listen to reaction_removed event
    unlistenPromises.push(
      listen<ReactionChangedPayload>("reaction_removed", (event) => {
        messagesState.removeReaction(
          event.payload.messageId,
          event.payload.contactId,
          event.payload.emoji,
          event.payload.contactId,
        );
      }),
    );

    // Listen to peer_id_rotated event (OUR OWN rotation was published)
    unlistenPromises.push(
      listen<string[]>("peer_id_rotated", (event) => {
        const count = event.payload?.length ?? 0;
        console.log(
          "\n\n" +
            "╔══════════════════════════════════════════╗\n" +
            "║         OWN PEER ID ROTATED              ║\n" +
            "╚══════════════════════════════════════════╝\n" +
            `  new addresses published: ${count}\n` +
            (event.payload ?? []).map((a, i) => `  [${i}] ${a}`).join("\n") +
            "\n",
        );
        notifications.push(
          count > 0
            ? `Peer ID rotated. ${count} new address${count === 1 ? "" : "es"} published.`
            : "Peer ID rotated.",
          "success",
        );
        // Reload contacts so their peerId fields reflect the new peer IDs
        contactsState.load();
      }),
    );

    // Listen to contact_updated event (Either peer ID rotated, OR profile updated)
    unlistenPromises.push(
      listen<ContactResponse>("contact_updated", (event) => {
        const payload = event.payload;
        const existing = contactsState.getById(payload.userId);

        // Save old values to compare, because upsert modifies Svelte 5 state in-place
        const oldPeerId = existing?.peerId;
        const oldName = existing?.displayName;
        const oldAvatar = existing?.avatarBase64;

        // Upsert handles avatar base64 decoding automatically
        contactsState.upsert(payload);

        if (!existing) return;

        const newContact = contactsState.getById(payload.userId);
        if (!newContact) return;

        if (oldPeerId && oldPeerId !== newContact.peerId) {
          notifications.push(
            `${newContact.displayName} rotated their peer ID.`,
            "info",
          );
        }

        const nameChanged = oldName && oldName !== newContact.displayName;
        const avatarChanged =
          oldAvatar !== newContact.avatarBase64 && newContact.avatarBase64;

        if (nameChanged && avatarChanged) {
          notifications.push(
            `${oldName} changed their name to ${newContact.displayName} and updated their profile picture`,
            "info",
          );
        } else if (nameChanged) {
          notifications.push(
            `${oldName} changed their name to ${newContact.displayName}`,
            "info",
          );
        } else if (avatarChanged) {
          notifications.push(
            `${newContact.displayName} updated their profile picture`,
            "info",
          );
        }
      }),
    );

    // Listen to ltc_expiring_soon event
    unlistenPromises.push(
      listen<number>("ltc_expiring_soon", (event) => {
        const hours = event.payload;
        notifications.push(
          `Long-term code expires in ${hours} hour${hours === 1 ? "" : "s"}.`,
          "info",
        );
      }),
    );

    // Listen to file_offered event
    unlistenPromises.push(
      listen<FileOfferedPayload>("file_offered", async (event) => {
        const payload = event.payload;
        messagesState.append({
          id: payload.offerId,
          contactId: payload.contactId,
          direction: "received",
          content: "[File]",
          status: "delivered",
          timestamp: Date.now(),
          replyTo: null,
          fileDetails: {
            filename: payload.filename,
            sizeBytes: payload.sizeBytes,
            autodownloadPath: payload.autodownload,
          },
        });
        messagesState.setFirstUnread(payload.contactId, payload.offerId);

        const senderName =
          contactsState.getById(payload.contactId)?.displayName ?? "Someone";
        const onThisChat =
          !document.hidden &&
          $page.url.pathname === `/chat/${payload.contactId}`;
        if (!onThisChat) {
          void notifyMessage({
            senderName,
            body: `Sent a file: ${payload.filename}`,
          });
        }

        if (payload.autodownload) {
          try {
            await acceptFileOffer(
              payload.contactId,
              payload.offerId,
              payload.autodownload,
            );
          } catch (e) {
            messagesState.setAutodownloadPath(
              payload.offerId,
              payload.contactId,
              null,
            );
            const reason = e instanceof Error ? e.message : String(e);
            notifications.push(`Auto-download failed: ${reason}`, "error");
            console.error("Auto-accept file offer failed", e);
          }
        }
      }),
    );

    // Listen to file transfer progress
    unlistenPromises.push(
      listen<FileTransferProgressPayload>("file_transfer_progress", (event) => {
        const { transferId, bytesTransferred, totalBytes } = event.payload;
        messagesState.setTransferProgress(
          transferId,
          bytesTransferred,
          totalBytes,
        );
      }),
    );

    // Listen to file_received event
    unlistenPromises.push(
      listen<FileReceivedPayload>("file_received", async (event) => {
        const { contactId, savePath } = event.payload;
        let filename = savePath.split(/[\\/]/).pop() || "file";

        try {
          const finalized = await finalizeDeferredReceiveTarget(savePath);
          filename = finalized.filename;
        } catch (err) {
          notifications.push(
            "Received file but could not place it in the selected location",
            "error",
          );
          console.error("Deferred receive finalization failed", err);
        }

        const contact = contactsState.getById(contactId);
        const contactName = contact?.displayName ?? "Contact";
        notifications.push(
          `Received ${filename} from ${contactName}`,
          "success",
        );
      }),
    );

    // Listen to nearby_request event
    unlistenPromises.push(
      listen<NearbyRequestPayload>("nearby_request", (event) => {
        nearbyState.addPendingRequest(
          event.payload.peerId,
          event.payload.sessionName,
        );
      }),
    );

    // Listen to typing_indicator event
    unlistenPromises.push(
      listen<TypingIndicatorPayload>("typing_indicator", (event) => {
        typingState.set(event.payload.contactId, event.payload.replyTo ?? null);
      }),
    );

    return () => {
      document.removeEventListener("visibilitychange", handleVisibilityChange);
      backgroundUnread = 0;
      refreshTitle();
      void Promise.all(unlistenPromises).then((fns) => {
        fns.forEach((fn) => fn());
      });
    };
  });
</script>

{@render children()}
<ToastContainer />
<ConfirmDialog />
