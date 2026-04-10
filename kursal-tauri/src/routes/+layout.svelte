<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { listen } from "@tauri-apps/api/event";
  import { goto } from "$app/navigation";
  import { contactsState } from "$lib/state/contacts.svelte";
  import { messagesState } from "$lib/state/messages.svelte";
  import { nearbyState } from "$lib/state/nearby.svelte";
  import { notifications } from "$lib/state/notifications.svelte";
  import ToastContainer from "$lib/components/ToastContainer.svelte";
  import type {
    MessageReceivedPayload,
    ConnectionChangedPayload,
    NearbyRequestPayload,
    ContactResponse,
    MessageEditedPayload,
    MessageDeletedPayload,
    ReactionChangedPayload,
  } from "$lib/types";

  let { children } = $props();
  let backgroundUnread = 0;
  let baseTitle = "Kursal";

  function refreshTitle() {
    document.title =
      backgroundUnread > 0 ? `(${backgroundUnread}) ${baseTitle}` : baseTitle;
  }

  onMount(() => {
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
    if (onboarded !== "true" && $page.url.pathname !== "/onboarding") {
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

    // Listen to message_received event
    unlistenPromises.push(
      listen<MessageReceivedPayload>("message_received", (event) => {
        const payload = event.payload;
        payload.timestamp = payload.timestamp * 1000; // Rust gives seconds, UI expects ms
        messagesState.append(payload);
        if (document.hidden) {
          backgroundUnread += 1;
          refreshTitle();
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
          event.payload.contactId
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
          event.payload.contactId
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
          `Contact file expires in ${hours} hour${hours === 1 ? "" : "s"}.`,
          "info",
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
