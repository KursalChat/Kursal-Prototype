<script lang="ts">
  import type { ConnectionChangedPayload } from '$lib/types';
  let { status }: { status: ConnectionChangedPayload['status'] } = $props();

  const colors: Record<ConnectionChangedPayload['status'], string> = {
    connecting:   '#fbbf24',
    relay:        '#38bdf8',
    holepunch:    '#38bdf8',
    direct:       '#34d399',
    disconnected: '#64748b',
  };
</script>

<span class="status-dot" style="background:{colors[status] ?? colors.disconnected}"></span>

<style>
  .status-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
    display: inline-block;
    border: 2px solid var(--bg-secondary);
    box-sizing: content-box;
  }
</style>
