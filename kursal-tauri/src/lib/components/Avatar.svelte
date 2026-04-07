<script lang="ts">
  let { name, size = 36, src = null }: { name: string; size?: number; src?: string | null } = $props();

  // Deterministic hue from name
  const hue = $derived([...name].reduce((a, c) => a + c.charCodeAt(0), 0) % 360);
  const initials = $derived(
    name.split(' ').map(w => w[0]?.toUpperCase() ?? '').slice(0, 2).join('')
  );

  const imgSrc = $derived(
    src ? (src.startsWith('data:') ? src : `data:image/webp;base64,${src}`) : null
  );
</script>

<div
  class="avatar"
  style="width:{size}px;height:{size}px;font-size:{size * 0.38}px;
         {imgSrc ? '' : `background:hsl(${hue},45%,30%);color:hsl(${hue},70%,85%)`}"
>
  {#if imgSrc}
    <img src={imgSrc} alt="{name}'s avatar" draggable="false" />
  {:else}
    {initials || '?'}
  {/if}
</div>

<style>
  .avatar {
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: 600;
    flex-shrink: 0;
    overflow: hidden;
    position: relative;
    user-select: none;
  }

  .avatar img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
</style>
