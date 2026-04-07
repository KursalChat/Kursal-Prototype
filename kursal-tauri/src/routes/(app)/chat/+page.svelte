<script lang="ts">
</script>

<div class="empty">
  <div class="empty-card">
    <p class="kicker">Kursal</p>
    <p class="title">Choose a conversation</p>
    <p class="subtitle">Pick someone from the sidebar or add a new contact to start chatting.</p>
  </div>
</div>

<style>
  .empty {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    padding: 24px;
  }

  .empty-card {
    border: 1px dashed var(--border);
    background: linear-gradient(160deg, rgba(30, 41, 59, 0.64), rgba(15, 23, 42, 0.72));
    border-radius: 18px;
    padding: 22px;
    max-width: 460px;
    box-shadow: var(--glow);
  }

  .kicker {
    margin: 0 0 6px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--accent-hover);
  }

  .title {
    font-size: 24px;
    color: var(--text-primary);
    margin: 0 0 8px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }

  .subtitle {
    margin: 0;
    font-size: 15px;
    line-height: 1.5;
    color: var(--text-secondary);
  }
</style>
