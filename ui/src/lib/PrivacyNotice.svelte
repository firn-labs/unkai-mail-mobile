<script lang="ts">
  /**
   * The privacy notice — what this app does with personal data, in
   * words someone can check.
   *
   * Why it lives *inside* the app, not only on a web page: App Store
   * guideline 5.1.1 requires the privacy policy to be reachable from
   * within the app, and GDPR Art. 12 asks for the information to be
   * easy to access at the point where data is processed. For a mail
   * client that point is the device.
   *
   * Every sentence here is a claim about the code, and each one was
   * checked against it when written:
   *
   *   * "no tracking / analytics" — the workspace contains no analytics,
   *     crash-reporting or advertising SDK, and no endpoint of the
   *     developers';
   *   * account setup — `unkai-discovery/src/autoconfig.rs` sends the
   *     full address to the provider's own autoconfig host and only
   *     the domain to `autoconfig.thunderbird.net`;
   *   * link check — `unkai-commands/src/mail.rs` downloads the URLhaus
   *     list and matches locally; `link_check_enabled` defaults to on;
   *   * mail always over TLS — `unkai-imap` only connects through
   *     `tls_connect`, `unkai-smtp` uses implicit TLS or required
   *     STARTTLS;
   *   * account removal — `remove_account` / `remove_nextcloud_account`
   *     delete the keychain entries and wipe the cached rows.
   *
   * **If you change any of those behaviours, change this text in the
   * same PR.** A privacy notice that has drifted from the code is worse
   * than none: it is a false statement to the user.
   *
   * Rendered in two places: the Settings screen (`PrivacyScreen`) and a
   * full sheet from the first-run screen, before any account exists.
   */
  import * as api from './api'
  import Icon from './Icon.svelte'
  import { m } from '../paraglide/messages'
  import { toasts } from './toast.svelte'

  async function openSettings() {
    try {
      await api.system.openAppSettings()
    } catch (e) {
      toasts.error(m.mobile_privacy_open_settings(), e)
    }
  }
</script>

<div class="privacy">
  <section class="list-group privacy__card privacy__card--lead">
    <h2 class="privacy__title">
      <Icon name="lock" size={18} />
      {m.mobile_privacy_summary_title()}
    </h2>
    <p>{m.mobile_privacy_summary_body()}</p>
  </section>

  <section class="list-group privacy__card">
    <h2 class="privacy__title">{m.mobile_privacy_device_title()}</h2>
    <p>{m.mobile_privacy_device_body()}</p>
  </section>

  <section class="list-group privacy__card">
    <h2 class="privacy__title">{m.mobile_privacy_servers_title()}</h2>
    <p>{m.mobile_privacy_servers_body()}</p>
  </section>

  <section class="list-group privacy__card">
    <h2 class="privacy__title">{m.mobile_privacy_third_title()}</h2>
    <p>{m.mobile_privacy_third_setup()}</p>
    <p>{m.mobile_privacy_third_links()}</p>
    <p class="privacy__note">{m.mobile_privacy_third_ip()}</p>
  </section>

  <section class="list-group privacy__card">
    <h2 class="privacy__title">{m.mobile_privacy_images_title()}</h2>
    <p>{m.mobile_privacy_images_body()}</p>
  </section>

  <section class="list-group privacy__card">
    <h2 class="privacy__title">{m.mobile_privacy_permissions_title()}</h2>
    <p>{m.mobile_privacy_permissions_body()}</p>
    <button type="button" class="value-button privacy__action" onclick={openSettings}>
      {m.mobile_privacy_open_settings()}
      <Icon name="open-in-browser" size={16} />
    </button>
  </section>

  <section class="list-group privacy__card">
    <h2 class="privacy__title">{m.mobile_privacy_delete_title()}</h2>
    <p>{m.mobile_privacy_delete_body()}</p>
  </section>

  <section class="list-group privacy__card">
    <h2 class="privacy__title">{m.mobile_privacy_rights_title()}</h2>
    <p>{m.mobile_privacy_rights_body()}</p>
  </section>
</div>

<style>
  .privacy {
    padding-bottom: var(--s-8);
  }

  /* Prose cards: the grouped-list card, holding paragraphs instead of
     rows. Readable text wants a line length and a leading, not row
     metrics. */
  .privacy__card {
    padding: var(--s-4);
    font-size: var(--t-body);
    line-height: var(--t-body-lh);
    color: var(--ui-ink);
    -webkit-user-select: text;
    user-select: text;
  }

  .privacy__card p + p {
    margin-top: var(--s-3);
  }

  .privacy__card--lead {
    background: color-mix(in oklab, var(--color-primary-500) 6%, var(--ui-card));
  }

  .privacy__title {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    margin-bottom: var(--s-2);
    font-size: var(--t-headline);
    line-height: var(--t-headline-lh);
    font-weight: 600;
  }

  .privacy__note {
    color: var(--ui-ink-muted);
    font-size: var(--t-callout);
    line-height: var(--t-callout-lh);
  }

  .privacy__action {
    margin-top: var(--s-3);
    margin-left: calc(-1 * var(--s-2));
  }
</style>
