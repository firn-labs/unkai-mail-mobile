<script lang="ts">
  /** Centred activity indicator for a screen that has nothing to
   *  paint yet. Screens that DO have cached content show it and put
   *  the spinner in the nav bar's hairline instead — a spinner over
   *  readable content is a downgrade. */
  import Icon from '../Icon.svelte'
  import { m } from '../../paraglide/messages'

  interface Props {
    label?: string
    /** Fill the available space rather than sitting inline. */
    full?: boolean
  }
  let { label = '', full = true }: Props = $props()
</script>

<!-- A status region, so VoiceOver announces that something is loading
     instead of the screen just going quiet. Without a visible label
     the announcement falls back to the generic "Loading…". -->
<div
  class="flex flex-col items-center justify-center gap-2 ink-muted"
  class:flex-1={full}
  class:py-8={full}
  role="status"
>
  <span class="spin"><Icon name="loading" size={22} /></span>
  {#if label}<p class="text-sm">{label}</p>{:else}<span class="sr-only">{m.mobile_loading()}</span>{/if}
</div>

<style>
  .spin {
    display: inline-flex;
    animation: spin 900ms linear infinite;
  }
  @keyframes spin {
    to { transform: rotate(360deg); }
  }
</style>
