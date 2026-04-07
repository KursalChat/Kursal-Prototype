<script lang="ts">
  import type { ConnectionChangedPayload } from '$lib/types';
  let { status }: { status: ConnectionChangedPayload['status'] } = $props();

  const colors: Record<ConnectionChangedPayload['status'], string> = {
    connecting:   '#faa81a',
    relay:        '#00b4d8',
    holepunch:    '#00b4d8',
    direct:       '#23a559',
    disconnected: '#4e5058',
  };
</script>

<span class="dot" style="background:{colors[status] ?? colors.disconnected}"></span>

<style>
  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
    display: inline-block;
  }
</style>
