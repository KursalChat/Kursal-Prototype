<script lang="ts">
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';

  onMount(() => {
    const onboarded = localStorage.getItem('kursal_onboarded');
    goto(onboarded ? '/chat' : '/onboarding', { replaceState: true });
  });
</script>
