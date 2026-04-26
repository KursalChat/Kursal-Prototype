<script lang="ts">
  import { page } from '$app/stores';
</script>

<div class="error-container">
  <div class="error-content">
    <h1>{$page.status}</h1>
    <p class="message">{$page.error?.message}</p>
  </div>
</div>

<style>
  .error-container {
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
  }

  .error-content {
    text-align: center;
    max-width: 400px;
  }

  h1 {
    margin: 0 0 16px 0;
    font-size: 32px;
  }

  .message {
    margin: 0 0 24px 0;
    color: var(--text-secondary);
    font-size: 14px;
    line-height: 1.6;
  }
</style>
