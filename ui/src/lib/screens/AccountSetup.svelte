<script lang="ts">
  /**
   * Adding a mail account.
   *
   * Three steps, and the middle one usually doesn't happen:
   *   1. Address + password.
   *   2. Autodiscovery (Mozilla autoconfig, SRV records, provider
   *      presets). If it answers, the server fields never appear.
   *   3. Manual server details, shown only when discovery came back
   *      empty or the connection test failed.
   *
   * Self-hosted servers add a fourth step that only appears when it
   * has to: if the TLS handshake fails because the certificate isn't
   * publicly signed, we probe the certificate the server actually
   * presented, show its SHA-256 fingerprint, and let the user trust
   * it explicitly. The trusted certificate is stored on the account,
   * so IMAP and SMTP both validate against it from then on.
   *
   * Doubles as the first-run screen — when there are no accounts at
   * all the shell mounts this directly, without a nav stack.
   */
  import * as api from '../api'
  import type { DiscoveredAccount, TrustedCert } from '../api'
  import Icon from '../Icon.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import Sheet from '../ui/Sheet.svelte'
  import PrivacyNotice from '../PrivacyNotice.svelte'
  import { accountsStore } from '../accountsStore.svelte'
  import { toasts } from '../toast.svelte'
  import { formatError, isCertificateError } from '../errors'
  import { m } from '../../paraglide/messages'

  interface Props {
    /** First run has nothing to go back to. */
    firstRun?: boolean
    onclose: (added: boolean) => void
  }

  let { firstRun = false, onclose }: Props = $props()

  /** The privacy notice, readable before any data is entered. */
  let showPrivacy = $state(false)

  /* ── Demo account ────────────────────────────────────────────── */

  let demoBusy = $state(false)

  /**
   * Seed the demo data, then hand control back exactly as a real
   * account would — `onclose(true)` is what makes the shell reload its
   * accounts and leave setup, so the demo needs no special path
   * through the boot sequence.
   */
  async function loadDemo() {
    demoBusy = true
    try {
      const summary = await api.accounts.createDemoAccount()
      toasts.success(
        m.mobile_demo_done({
          messages: summary.messages,
          contacts: summary.contacts,
          events: summary.events,
        }),
      )
      onclose(true)
    } catch (e) {
      toasts.error(m.mobile_demo_failed(), e)
    } finally {
      demoBusy = false
    }
  }

  let email = $state('')
  let password = $state('')
  let displayName = $state('')
  let personName = $state('')

  let imapHost = $state('')
  let imapPort = $state(993)
  let smtpHost = $state('')
  let smtpPort = $state(465)

  let manual = $state(false)
  let passwordInput: HTMLInputElement | undefined = $state()
  let busy = $state(false)
  let status = $state('')
  let error = $state('')

  /** Certificates the user explicitly trusted for this server. */
  let trustedCerts = $state<TrustedCert[]>([])
  /** A certificate the server presented that nothing vouches for.
   *  Non-null puts the trust prompt on screen. */
  let untrusted = $state<{ sha256: string; der: number[]; host: string } | null>(null)

  /** Ask the server for the certificate it presents, without
   *  validating it, so the user can decide for themselves. */
  async function probeCertificate() {
    try {
      const probed = await api.accounts.probeServerCertificate({
        host: imapHost,
        port: imapPort,
      })
      const leaf = probed.chain?.[0]
      if (!leaf) return false
      untrusted = { sha256: leaf.sha256, der: leaf.der, host: probed.host }
      return true
    } catch {
      return false
    }
  }

  function trustCertificate() {
    if (!untrusted) return
    trustedCerts = [
      ...trustedCerts,
      {
        der: untrusted.der,
        sha256: untrusted.sha256,
        host: untrusted.host,
        added_at: Math.floor(Date.now() / 1000),
      },
    ]
    untrusted = null
    error = ''
    void submit()
  }

  const canSubmit = $derived(
    email.includes('@') &&
      password.length > 0 &&
      (!manual || (imapHost.length > 0 && smtpHost.length > 0)) &&
      !busy,
  )

  async function discover(): Promise<DiscoveredAccount | null> {
    status = m.mobile_setup_discovering()
    try {
      return await api.accounts.discoverAccountSettings({ email })
    } catch {
      // Discovery failing is expected on custom domains; the manual
      // form is the answer, not an error message.
      return null
    }
  }

  async function submit() {
    if (!canSubmit) return
    busy = true
    error = ''
    try {
      if (!manual) {
        const found = await discover()
        if (found) {
          imapHost = found.imap_host
          imapPort = found.imap_port
          smtpHost = found.smtp_host
          smtpPort = found.smtp_port
        } else {
          manual = true
          status = ''
          error = m.mobile_setup_discovery_failed()
          busy = false
          return
        }
      }

      status = m.mobile_setup_testing()
      await api.accounts.testConnection({
        host: imapHost,
        port: imapPort,
        username: email,
        password,
        trustedCerts: trustedCerts.length > 0 ? trustedCerts : null,
      })

      status = m.mobile_setup_saving()
      await api.accounts.addAccount({
        account: {
          id: crypto.randomUUID(),
          display_name: displayName.trim() || email,
          email,
          imap_host: imapHost,
          imap_port: imapPort,
          smtp_host: smtpHost,
          smtp_port: smtpPort,
          person_name: personName.trim() || null,
          trusted_certs: trustedCerts,
        },
        password,
      })
      await accountsStore.load()
      toasts.success(m.mobile_setup_added())
      onclose(true)
    } catch (e) {
      status = ''
      // A TLS failure isn't the user's mistake to fix in a text
      // field — offer the certificate instead.
      if (isCertificateError(e) && untrusted === null && (await probeCertificate())) {
        busy = false
        return
      }
      // Otherwise: surface it and open the manual fields, because a
      // wrong password and a wrong host look the same from here and
      // both are fixed on this screen.
      error = formatError(e)
      manual = true
    } finally {
      busy = false
    }
  }
</script>

<div class="setup-shell">
  <div class="screen">
    <NavBar title={firstRun ? '' : m.mobile_add_account()}>
      {#snippet leading()}
        {#if firstRun}
          <!-- Test mode lives in the corner a thumb reaches first and the
               eye reads first, as a clear pill rather than a card that
               pushed the sign-in form down the screen. What test mode
               *is* is explained by the introduction it opens into. -->
          <button
            type="button"
            class="demo-pill"
            aria-label={m.mobile_demo_pill_sr()}
            disabled={demoBusy}
            onclick={loadDemo}
          >
            <span class="demo-pill__icon" class:demo-pill__icon--busy={demoBusy}>
              <Icon name={demoBusy ? 'loading' : 'eye'} size={16} />
            </span>
            <span>{demoBusy ? m.mobile_demo_working() : m.mobile_demo_pill()}</span>
          </button>
        {:else}
          <button type="button" class="bar-button bar-button--text" onclick={() => onclose(false)}>
            {m.mobile_cancel()}
          </button>
        {/if}
      {/snippet}
      <!-- No "Next" in the bar: the screen has one primary action, the
           labelled button under the form, and the keyboard's Go key
           submits too. The bar's copy was a second, smaller button for
           the same thing. -->
    </NavBar>

    <div class="screen__body form-group">
      {#if firstRun}
        <div class="setup-hero">
          <img src="unkai-logo://storm" alt="" class="setup-hero__logo" />
          <h1 class="setup-hero__title">{m.mobile_setup_welcome()}</h1>
          <p class="setup-hero__body">{m.mobile_setup_welcome_body()}</p>
          <p class="demo-hint">{m.mobile_demo_hint()}</p>
        </div>
      {/if}

      <!-- The fields sit on the page, each under its own visible label —
           no card around them. v3 nested outlined fields inside a card,
           which put three surfaces and two border colours into one small
           form. A placeholder is not a label (it vanishes on the first
           keystroke), so every field has a real one. -->
      <div class="setup-form">
        <label class="setup-field">
          <span class="setup-field__label">{m.mobile_setup_email_label()}</span>
          <input
            type="email"
            inputmode="email"
            autocapitalize="none"
            autocorrect="off"
            spellcheck="false"
            autocomplete="username"
            enterkeyhint="next"
            bind:value={email}
            placeholder={m.mobile_setup_email_placeholder()}
            onkeydown={(e) => e.key === 'Enter' && (e.preventDefault(), passwordInput?.focus())}
          />
        </label>
        <label class="setup-field">
          <span class="setup-field__label">{m.mobile_setup_password_label()}</span>
          <input
            type="password"
            autocomplete="current-password"
            enterkeyhint="go"
            bind:value={password}
            bind:this={passwordInput}
            aria-describedby="setup-password-hint"
            onkeydown={(e) => e.key === 'Enter' && (e.preventDefault(), void submit())}
          />
          <span class="setup-field__hint" id="setup-password-hint">{m.mobile_setup_password_hint()}</span>
        </label>
        <label class="setup-field">
          <span class="setup-field__label">{m.mobile_setup_name_label()}</span>
          <input
            type="text"
            autocomplete="name"
            enterkeyhint="done"
            bind:value={personName}
            placeholder={m.mobile_setup_name_hint()}
          />
        </label>
      </div>

      {#if manual}
        <h2 class="section-title">{m.mobile_setup_servers()}</h2>
        <div class="setup-form">
          <div class="setup-field">
            <span class="setup-field__label" id="setup-imap-label">{m.mobile_setup_imap_host()}</span>
            <div class="field-pair">
              <input
                type="text"
                class="field-pair__grow"
                autocapitalize="none"
                autocorrect="off"
                spellcheck="false"
                bind:value={imapHost}
                aria-labelledby="setup-imap-label"
                placeholder="imap.example.com"
              />
              <input
                type="number"
                inputmode="numeric"
                class="field-pair__port"
                aria-label={m.mobile_setup_imap_port()}
                bind:value={imapPort}
              />
            </div>
          </div>
          <div class="setup-field">
            <span class="setup-field__label" id="setup-smtp-label">{m.mobile_setup_smtp_host()}</span>
            <div class="field-pair">
              <input
                type="text"
                class="field-pair__grow"
                autocapitalize="none"
                autocorrect="off"
                spellcheck="false"
                bind:value={smtpHost}
                aria-labelledby="setup-smtp-label"
                placeholder="smtp.example.com"
              />
              <input
                type="number"
                inputmode="numeric"
                class="field-pair__port"
                aria-label={m.mobile_setup_smtp_port()}
                bind:value={smtpPort}
              />
            </div>
          </div>
          <label class="setup-field">
            <span class="setup-field__label">{m.mobile_setup_display_name_label()}</span>
            <input type="text" bind:value={displayName} placeholder={m.mobile_setup_display_name_hint()} />
          </label>
          <p class="setup-field__hint">{m.mobile_setup_tls_note()}</p>
        </div>
      {/if}

      {#if status}
        <p class="px-4 py-3 text-center t-footnote ink-muted" role="status">
          <span class="inline-flex items-center gap-2">
            <Icon name="loading" size={14} />
            {status}
          </span>
        </p>
      {/if}

      {#if untrusted}
        <div class="setup-notice setup-notice--warning">
          <div class="flex items-center gap-2">
            <span class="ink-warning inline-flex"><Icon name="warning" size={18} /></span>
            <h2 class="t-callout font-semibold">{m.mobile_setup_cert_title()}</h2>
          </div>
          <p class="mt-1 t-footnote">{m.mobile_setup_cert_body({ host: untrusted.host })}</p>
          <p class="setup-notice__code selectable">
            {untrusted.sha256}
          </p>
          <p class="mt-2 t-footnote ink-muted">{m.mobile_setup_cert_hint()}</p>
          <div class="mt-3 flex gap-2">
            <button type="button" class="wide-button" onclick={trustCertificate}>
              {m.mobile_setup_cert_trust()}
            </button>
            <button
              type="button"
              class="wide-button wide-button--quiet"
              onclick={() => (untrusted = null)}
            >
              {m.mobile_cancel()}
            </button>
          </div>
        </div>
      {/if}

      {#if trustedCerts.length > 0 && !untrusted}
        <p class="list-footer ink-success">{m.mobile_setup_cert_trusted()}</p>
      {/if}

      {#if error}
        <p class="setup-notice setup-notice--danger t-callout" role="alert">{error}</p>
      {/if}

      <div class="page-inset pb-10 pt-4">
        <button type="button" class="wide-button" disabled={!canSubmit} onclick={submit}>
          {busy ? m.mobile_working() : m.mobile_setup_add()}
        </button>
        {#if !manual}
          <!-- Plain, not a grey pill: one filled button is the action;
               this is the way round it for the few who need it. -->
          <button
            type="button"
            class="wide-button wide-button--plain mt-1"
            onclick={() => (manual = true)}
          >
            {m.mobile_setup_manual()}
          </button>
        {/if}
        <!-- Before the user types an address or a password, what happens
             to them has to be one tap away (App Store 5.1.1, GDPR Art. 13
             — information at the point of collection). -->
        <div class="mt-4 flex justify-center">
          <button type="button" class="value-button" onclick={() => (showPrivacy = true)}>
            <Icon name="eye-off" size={16} />
            {m.mobile_settings_privacy()}
          </button>
        </div>
      </div>
    </div>
  </div>
</div>

{#if showPrivacy}
  <Sheet kind="full" label={m.mobile_settings_privacy()} onclose={() => (showPrivacy = false)}>
    <div class="screen">
      <NavBar title={m.mobile_settings_privacy()}>
        {#snippet actions()}
          <button
            type="button"
            class="bar-button bar-button--text bar-button--strong"
            onclick={() => (showPrivacy = false)}
          >
            {m.mobile_close()}
          </button>
        {/snippet}
      </NavBar>
      <div class="screen__body">
        <PrivacyNotice />
      </div>
    </div>
  </Sheet>
{/if}

<style>
  /* The test-mode pill: the accent's solved fill and label pair, so it
     stays legible on every theme; 44pt tall to hit, like every target
     in the bar. */
  .demo-pill {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: var(--s-1);
    min-height: 36px;
    margin-block: 4px;
    margin-left: var(--s-2);
    padding: 0 var(--s-3);
    border-radius: var(--r-full);
    background: var(--ui-accent-fill);
    color: var(--ui-on-accent);
    font-size: var(--t-bar-body);
    font-weight: 600;
    transition: transform var(--m-fast) var(--m-ease);
  }

  /* 44pt target around the 36px pill. */
  .demo-pill::before {
    content: '';
    position: absolute;
    inset: -4px -2px;
  }

  .demo-pill:active:not(:disabled) {
    transform: scale(0.96);
  }

  .demo-pill:disabled {
    opacity: 0.7;
  }

  .demo-pill__icon {
    display: inline-flex;
  }

  .demo-pill__icon--busy {
    animation: demo-pill-spin 900ms linear infinite;
  }

  @keyframes demo-pill-spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* The first thing a new user sees. v3 sets it like the opening of a
     letter rather than a landing page: left-aligned on the gutter, the
     mark at the size of an app icon, the welcome in the display face.
     v2 centred everything and floated the logo on a card with a shadow
     — the default hero of every template, and it pushed the form a
     screen down. */
  .setup-hero {
    padding: var(--s-4) var(--gutter) var(--s-2);
  }

  .setup-hero__logo {
    width: 56px;
    height: 56px;
    border-radius: 13px;
  }

  .setup-hero__title {
    margin-top: var(--s-5);
    font-family: var(--font-display);
    font-size: var(--t-display);
    line-height: var(--t-display-lh);
    font-weight: 800;
    letter-spacing: -0.028em;
    color: var(--ui-ink);
    text-wrap: pretty;
      overflow-wrap: break-word;
    -webkit-hyphens: auto;
    hyphens: auto;
  }

  .setup-hero__body {
    margin-top: var(--s-3);
    max-width: 30ch;
    font-size: var(--t-body);
    line-height: var(--t-body-lh);
    color: var(--ui-ink-muted);
  }

  .demo-hint {
    margin-top: var(--s-3);
    font-size: var(--t-footnote);
    line-height: var(--t-footnote-lh);
    color: var(--ui-ink-muted);
  }

  /* ── The form ─────────────────────────────────────────────────
     Label above field, fields on the page. One surface, one outline
     colour (`--ui-control`, 3:1 against the page — WCAG 1.4.11), and
     the accent only where focus is. */
  .setup-form {
    display: flex;
    flex-direction: column;
    gap: var(--s-4);
    padding: var(--s-4) var(--gutter) 0;
  }

  .setup-field {
    display: flex;
    flex-direction: column;
    gap: var(--s-1);
  }

  .setup-field__label {
    font-size: var(--t-footnote);
    line-height: var(--t-footnote-lh);
    font-weight: 600;
    color: var(--ui-ink-muted);
  }

  /* A step taller and rounder than a settings field: these are the
     first controls a new user touches, typed into with a thumb. */
  .setup-field input {
    min-height: 50px;
    border-radius: var(--r-md);
    padding-inline: var(--s-4);
  }

  .setup-field__hint {
    font-size: var(--t-footnote);
    line-height: var(--t-footnote-lh);
    color: var(--ui-ink-muted);
  }

  /* Inline notices: ink on a role tint, never role-coloured body text. */
  .setup-notice {
    margin: var(--s-3) var(--gutter);
    padding: var(--s-3) var(--s-4);
    border-radius: var(--r-md);
    color: var(--ui-ink);
  }

  .setup-notice--warning {
    background: var(--ui-tint-warning);
  }

  .setup-notice--danger {
    background: var(--ui-tint-danger);
  }

  .setup-notice__code {
    margin-top: var(--s-2);
    padding: var(--s-2);
    border-radius: var(--r-sm);
    background: var(--ui-fill-quiet);
    font-family: ui-monospace, 'SF Mono', Menlo, monospace;
    font-size: var(--t-micro);
    line-height: var(--t-micro-lh);
    word-break: break-all;
  }

  /**
   * Account setup covers the whole app, tab bar included.
   *
   * It is mounted two ways: as the first-run screen (nothing else on
   * screen) and as a modal over the running app when a second
   * account is added from Settings. The second case is why this
   * wrapper exists — a plain `.screen` in normal document flow lands
   * *below* the app's own full-height shell, so the Add button
   * looked like it did nothing at all.
   */
  .setup-shell {
    position: fixed;
    inset: 0;
    /* Ends above the keyboard (`lib/keyboardViewport.ts`). */
    bottom: var(--keyboard-inset, 0px);
    z-index: 58;
    background: var(--ui-canvas);
  }
</style>
