<script lang="ts">
  import { goto } from "$app/navigation";
  import { onMount } from "svelte";

  let startupError = "";

  onMount(() => {
    try {
      const onboarded = localStorage.getItem("kursal_onboarded");
      const target = onboarded == "done" ? "/chat" : "/onboarding";

      void goto(target, { replaceState: true }).catch((err) => {
        console.error("initial goto failed", err);
        window.location.replace(target);
      });
    } catch (err) {
      startupError = `Failed to initialize app route: ${String(err)}`;
      console.error("startup route init failed", err);
    }
  });
</script>

{#if startupError}
  <main style="padding: 1rem; font-family: sans-serif;">
    <h1>Kursal failed to start</h1>
    <p>{startupError}</p>
  </main>
{/if}
