<script lang="ts">
  /** Settings → Privacy. The notice itself is `PrivacyNotice`, shared
   *  with the first-run screen. */
  import NavBar from '../ui/NavBar.svelte'
  import PrivacyNotice from '../PrivacyNotice.svelte'
  import { nav } from '../nav.svelte'
  import { m } from '../../paraglide/messages'
</script>

<div class="screen">
  <NavBar
    title={m.mobile_settings_privacy()}
    backLabel={m.mobile_settings()}
    onback={() => nav.pop()}
    large
  />
  <div class="screen__body">
    <PrivacyNotice />
  </div>
</div>
