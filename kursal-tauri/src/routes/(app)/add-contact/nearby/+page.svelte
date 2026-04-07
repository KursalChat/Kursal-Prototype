<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { startNearby, stopNearby, getNearbyPeers, connectNearby, acceptNearby, declineNearby } from '$lib/api/nearby';
  import { nearbyState } from '$lib/state/nearby.svelte';
  import { notifications } from '$lib/state/notifications.svelte';
  import Button from '$lib/components/Button.svelte';

  let connecting = $state<Set<string>>(new Set());
  let pollInterval: ReturnType<typeof setInterval> | null = null;
  let isInitialLoad = $state(true);

  function setConnecting(peerId: string, isBusy: boolean) {
    const next = new Set(connecting);
    if (isBusy) next.add(peerId);
    else next.delete(peerId);
    connecting = next;
  }

  onMount(async () => {
    try {
      const sessionName = await startNearby();
      nearbyState.active = true;
      nearbyState.mySessionName = sessionName;

      const peers = await getNearbyPeers();
      nearbyState.setPeers(peers);
      isInitialLoad = false;

      pollInterval = setInterval(async () => {
        try {
          const nextPeers = await getNearbyPeers();
          nearbyState.setPeers(nextPeers);
        } catch (e) {
          console.error('Failed to get nearby peers:', e);
        }
      }, 3000);
    } catch (e) {
      notifications.push('Failed to start nearby discovery', 'error');
      console.error('Start nearby failed:', e);
      isInitialLoad = false;
    }
  });

  onDestroy(async () => {
    if (pollInterval) clearInterval(pollInterval);
    try {
      await stopNearby();
    } catch (e) {
      console.error('Stop nearby failed:', e);
    }
    nearbyState.reset();
  });

  async function handleConnect(peerId: string) {
    setConnecting(peerId, true);
    try {
      await connectNearby(peerId);
      notifications.push('Connection request sent', 'info');
    } catch (e) {
      notifications.push('Failed to connect to peer', 'error');
      console.error('Connect failed:', e);
    } finally {
      setConnecting(peerId, false);
    }
  }

  async function handleAccept(peerId: string) {
    setConnecting(peerId, true);
    try {
      await acceptNearby(peerId);
      nearbyState.removePendingRequest(peerId);
      notifications.push('Connection accepted', 'success');
    } catch (e) {
      notifications.push('Failed to accept connection', 'error');
      console.error('Accept failed:', e);
    } finally {
      setConnecting(peerId, false);
    }
  }

  async function handleDecline(peerId: string) {
    try {
      await declineNearby(peerId);
      nearbyState.removePendingRequest(peerId);
      notifications.push('Connection declined', 'info');
    } catch (e) {
      console.error('Decline failed:', e);
    }
  }
</script>

<div class="nearby-page">
  <section class="identity-card">
    <div>
      <p class="kicker">Your Nearby Name</p>
      <h3>{nearbyState.mySessionName || 'Starting discovery...'}</h3>
    </div>
    <span class="status" class:active={nearbyState.active}>
      {nearbyState.active ? 'Scanning' : 'Stopped'}
    </span>
  </section>

  {#if Object.keys(nearbyState.pendingRequests).length > 0}
    <section class="panel warning">
      <div class="panel-header">
        <h4>Incoming Requests</h4>
      </div>
      <div class="row-list">
        {#each Object.entries(nearbyState.pendingRequests) as [peerId, sessionName]}
          <div class="row-item">
            <div class="row-info">
              <p class="row-title">{sessionName}</p>
              <p class="row-subtitle">{peerId.slice(0, 14)}... wants to connect</p>
            </div>
            <div class="row-actions">
              <Button variant="secondary" loading={connecting.has(peerId)} onclick={() => handleDecline(peerId)}>
                Decline
              </Button>
              <Button variant="primary" loading={connecting.has(peerId)} onclick={() => handleAccept(peerId)}>
                Accept
              </Button>
            </div>
          </div>
        {/each}
      </div>
    </section>
  {/if}

  <section class="panel">
    <div class="panel-header">
      <h4>Devices Nearby</h4>
      <p>{nearbyState.peers.length} found</p>
    </div>

    {#if isInitialLoad}
      <div class="empty">Scanning local network...</div>
    {:else if nearbyState.peers.length === 0}
      <div class="empty">No devices found yet. Keep this page open on both devices.</div>
    {:else}
      <div class="row-list">
        {#each nearbyState.peers as peer (peer.peerId)}
          <div class="row-item">
            <div class="row-info">
              <p class="row-title">{peer.sessionName}</p>
              <p class="row-subtitle">{peer.peerId.slice(0, 14)}...</p>
            </div>
            <Button variant="primary" loading={connecting.has(peer.peerId)} onclick={() => handleConnect(peer.peerId)}>
              Connect
            </Button>
          </div>
        {/each}
      </div>
    {/if}
  </section>
</div>

<style>
  .nearby-page {
    max-width: 760px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .identity-card {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border: 1px solid var(--border);
    border-radius: 16px;
    background: linear-gradient(160deg, rgba(30, 41, 59, 0.64), rgba(15, 23, 42, 0.74));
    padding: 16px 18px;
    box-shadow: var(--glow);
  }

  .kicker {
    margin: 0;
    font-size: 11px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.12em;
  }

  .identity-card h3 {
    margin: 6px 0 0;
    font-size: 18px;
    letter-spacing: -0.01em;
  }

  .status {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-muted);
    border: 1px solid rgba(148, 163, 184, 0.3);
    background: rgba(30, 41, 59, 0.56);
    border-radius: 999px;
    padding: 6px 10px;
  }

  .status.active {
    color: #c7d2fe;
    border-color: rgba(129, 140, 248, 0.42);
    background: rgba(99, 102, 241, 0.16);
  }

  .panel {
    border: 1px solid var(--border);
    border-radius: 16px;
    background: linear-gradient(160deg, rgba(30, 41, 59, 0.64), rgba(15, 23, 42, 0.74));
    box-shadow: var(--glow);
    overflow: hidden;
  }

  .panel.warning {
    border-color: rgba(251, 191, 36, 0.36);
  }

  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid var(--border);
    padding: 12px 14px;
    background: rgba(15, 23, 42, 0.4);
  }

  .panel-header h4,
  .panel-header p {
    margin: 0;
  }

  .panel-header h4 {
    font-size: 13px;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--text-secondary);
  }

  .panel-header p {
    font-size: 12px;
    color: var(--text-muted);
  }

  .row-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px;
  }

  .row-item {
    border: 1px solid rgba(148, 163, 184, 0.22);
    border-radius: 12px;
    background: rgba(15, 23, 42, 0.58);
    padding: 12px;
    display: flex;
    justify-content: space-between;
    gap: 12px;
    align-items: center;
  }

  .row-info {
    min-width: 0;
  }

  .row-title {
    margin: 0;
    font-weight: 600;
    font-size: 14px;
    color: var(--text-primary);
  }

  .row-subtitle {
    margin: 3px 0 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .row-actions {
    display: flex;
    gap: 8px;
  }

  .empty {
    padding: 18px 14px;
    color: var(--text-muted);
    font-size: 14px;
  }

  @media (max-width: 640px) {
    .row-item {
      flex-direction: column;
      align-items: stretch;
    }

    .row-actions {
      width: 100%;
    }

    :global(.row-actions .button),
    :global(.row-item > .button) {
      width: 100%;
    }
  }
</style>
