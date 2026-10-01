<script lang="ts" module>
  export interface ComposeInitial {
    accountId?: string
    to?: string
    cc?: string
    bcc?: string
    subject?: string
    /** The user's own text (never the quoted history — see below). */
    body?: string
    /** Read-only HTML appended below the body at send time. */
    quotedHtml?: string
    /** Threading anchors, so replies group in the recipient's client. */
    inReplyTo?: string | null
    references?: string[]
    /** Marks the source message answered and picks the reply glyph. */
    repliedTo?: {
      accountId: string
      folder: string
      uid: number
      kind: 'reply' | 'reply-all' | 'forward'
    } | null
    /** Existing draft being edited — expunged after a successful send. */
    draftRef?: { accountId: string; folder: string; uid: number } | null
    attachments?: { filename: string; content_type: string; data: number[] }[]
    encrypt?: boolean
    /** Don't append the account signature (the body already has one:
     *  an edited draft, or a re-opened outbox entry). */
    skipSignature?: boolean
  }
</script>

<script lang="ts">
  /**
   * The composer.
   *
   * Deliberately a **plain-text editor with an HTML tail**, not a
   * rich-text canvas. Two reasons:
   *
   *   * A `contenteditable` under an iOS software keyboard is a
   *     minefield — selection handles, autocorrect replacements and
   *     the scroll-into-view dance all fight a custom editor, and
   *     what people type on a phone is prose, not layout.
   *   * The quoted history a reply carries must survive *byte for
   *     byte*: the sender's tables, inline styles and images. Keeping
   *     it out of the editor entirely (rendered below, spliced in at
   *     send time) is the only way to guarantee that. The desktop
   *     build does the same thing for the same reason.
   *
   * So the outgoing message is: the typed text (converted to HTML
   * with line breaks preserved), then the signature, then the
   * untouched quoted block.
   */
  import * as api from '../api'
  import Icon from '../Icon.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import ActionSheet, { type SheetAction } from '../ui/ActionSheet.svelte'
  import Confirm from '../ui/Confirm.svelte'
  import FileTypeIcon from '../FileTypeIcon.svelte'
  import NextcloudPicker from './NextcloudPicker.svelte'
  import { accountsStore } from '../accountsStore.svelte'
  import { toasts } from '../toast.svelte'
  import { fileSize } from '../format'
  import { textToHtml } from '../mail/renderHtml'
  import { mentionsAttachment } from '../attachmentMentions'
  import { m } from '../../paraglide/messages'

  interface Props {
    initial?: ComposeInitial
    onclose: () => void
  }

  let { initial = {}, onclose }: Props = $props()

  /* Seeded once, deliberately: the shell mounts the composer fresh
     every time it opens and unmounts it on close, so the form owns
     its fields from here on. */
  // svelte-ignore state_referenced_locally
  const seed = initial

  const accounts = $derived(accountsStore.list)

  let accountId = $state(seed.accountId ?? '')
  let to = $state(seed.to ?? '')
  let cc = $state(seed.cc ?? '')
  let bcc = $state(seed.bcc ?? '')
  let subject = $state(seed.subject ?? '')
  let body = $state(seed.body ?? '')
  let showCc = $state(!!(seed.cc || seed.bcc))
  let attachments = $state<{ filename: string; content_type: string; data: number[] }[]>(
    seed.attachments ?? [],
  )
  let encrypt = $state(seed.encrypt ?? false)
  let sending = $state(false)
  let showDiscard = $state(false)
  let moreOpen = $state(false)
  let ncPickerOpen = $state(false)
  let fileInput: HTMLInputElement | undefined = $state()
  /** "You wrote 'attached' but attached nothing" — asked once. */
  let confirmMissingAttachment = $state(false)

  const account = $derived(accounts.find((a) => a.id === accountId) ?? accounts[0])
  const signature = $derived(seed.skipSignature ? '' : (account?.signature ?? ''))
  const canSend = $derived(!!account && to.trim().length > 0 && !sending)

  $effect(() => {
    if (!accountId && accounts.length > 0) accountId = accounts[0].id
  })

  /* ── Recipient autocomplete ──────────────────────────────────── */

  /** Which field the suggestion list is attached to. */
  let activeField = $state<'to' | 'cc' | 'bcc' | null>(null)
  let suggestions = $state<{ label: string; address: string }[]>([])

  function currentFragment(value: string): string {
    const parts = value.split(',')
    return parts[parts.length - 1].trim()
  }

  async function updateSuggestions(field: 'to' | 'cc' | 'bcc', value: string) {
    activeField = field
    const fragment = currentFragment(value)
    if (fragment.length < 2) {
      suggestions = []
      return
    }
    try {
      const hits = await api.contacts.searchContacts({ query: fragment, limit: 8 })
      suggestions = hits
        .flatMap((c: { display_name: string; email: { value: string }[] }) =>
          (c.email ?? []).map((e) => ({ label: c.display_name, address: e.value })),
        )
        .filter((s: { address: string }) => s.address)
        .slice(0, 8)
    } catch {
      // Contacts may not be synced yet — typing the address still works.
      suggestions = []
    }
  }

  function applySuggestion(suggestion: { label: string; address: string }) {
    const formatted = suggestion.label
      ? `${suggestion.label} <${suggestion.address}>`
      : suggestion.address
    const replaceLast = (value: string) => {
      const parts = value.split(',')
      parts[parts.length - 1] = ` ${formatted}`
      return `${parts.join(',').trim()}, `
    }
    if (activeField === 'to') to = replaceLast(to)
    else if (activeField === 'cc') cc = replaceLast(cc)
    else if (activeField === 'bcc') bcc = replaceLast(bcc)
    suggestions = []
  }

  /* ── Attachments ─────────────────────────────────────────────── */

  /**
   * Read picked files through a `FileReader` rather than `btoa` over
   * a spread: a multi-MB image blows the argument limit of
   * `String.fromCharCode(...bytes)`, and this path is async anyway.
   *
   * A plain `<input type="file">` is used on purpose — in a WKWebView
   * it opens the native sheet with Photo Library, Camera and Files,
   * which is what people reach for on a phone. The document picker
   * behind `api.platform.openFileDialog` offers only Files.
   */
  function onPickFiles(e: Event) {
    const input = e.currentTarget as HTMLInputElement
    const files = Array.from(input.files ?? [])
    input.value = ''
    for (const file of files) {
      const reader = new FileReader()
      reader.onload = () => {
        const buffer = reader.result as ArrayBuffer
        attachments = [
          ...attachments,
          {
            filename: file.name,
            content_type: file.type || 'application/octet-stream',
            data: Array.from(new Uint8Array(buffer)),
          },
        ]
      }
      reader.onerror = () => toasts.error(m.mobile_attachment_read_failed({ name: file.name }))
      reader.readAsArrayBuffer(file)
    }
  }

  function removeAttachment(index: number) {
    attachments = attachments.filter((_, i) => i !== index)
  }

  const attachmentBytes = $derived(attachments.reduce((sum, a) => sum + a.data.length, 0))

  /**
   * A Nextcloud share link instead of a file.
   *
   * The reason this app exists, on the send side: a 40 MB video is
   * a link, not an attachment, and the recipient gets it from the
   * user's own server rather than a third party's.
   */
  async function attachFromNextcloud(ncId: string, path: string) {
    ncPickerOpen = false
    try {
      const share = await api.nextcloud.createNextcloudShare({ ncId, path })
      const name = path.split('/').pop() ?? path
      if (!share.url) throw new Error('the share came back without a URL')
      body = `${body}${body.endsWith('\n') || body === '' ? '' : '\n'}\n${name}: ${share.url}\n`
      toasts.success(m.mobile_share_link_added())
    } catch (e) {
      toasts.error(m.mobile_share_link_failed(), e)
    }
  }

  /* ── Send / save ─────────────────────────────────────────────── */

  function splitAddresses(value: string): string[] {
    return value
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean)
  }

  function buildOutgoing() {
    const signatureHtml = signature
      ? `<br><br><div class="unkai-signature">${signature}</div>`
      : ''
    const quoted = seed.quotedHtml ? `<br>${seed.quotedHtml}` : ''
    return {
      from: account?.email ?? '',
      to: splitAddresses(to),
      cc: splitAddresses(cc),
      bcc: splitAddresses(bcc),
      subject,
      body_text: body,
      body_html: `${textToHtml(body)}${signatureHtml}${quoted}`,
      attachments,
      in_reply_to: seed.inReplyTo ?? null,
      references: seed.references ?? [],
      encryption_mode: encrypt ? 'pgp' : null,
    }
  }

  async function send() {
    if (!canSend || !account) return

    // Demo accounts cannot send. The backend refuses too — that is the
    // real guard — but it answers in English, and user-facing text is
    // the frontend's job (backend strings stay English by convention).
    // Checking here means the user gets the message in their own
    // language, and gets it instantly rather than after a round trip.
    if (account.demo) {
      toasts.error(m.mobile_demo_hint_send())
      return
    }

    // The one send-time check worth interrupting for: the body talks
    // about an attachment and there isn't one. Asked once — a second
    // Send goes through regardless.
    if (attachments.length === 0 && !confirmMissingAttachment && mentionsAttachment(body)) {
      confirmMissingAttachment = true
      return
    }

    sending = true
    try {
      await api.compose.sendEmail({
        accountId: account.id,
        email: buildOutgoing(),
        repliedTo: seed.repliedTo ?? null,
        // An empty passphrase routes the backend through the
        // account's keychain entry when "unlock automatically" is on;
        // otherwise it surfaces an auth error the user can act on.
        pgpPassphrase: encrypt ? '' : null,
      })
      if (seed.draftRef) {
        try {
          await api.compose.expungeDraftAfterSend(seed.draftRef)
        } catch {
          // The message went out; a stale draft copy is cosmetic and
          // the next sync usually clears it.
        }
      }
      toasts.success(m.mobile_message_sent())
      onclose()
    } catch (e) {
      toasts.error(m.mobile_send_failed(), e)
    } finally {
      sending = false
    }
  }

  async function saveDraft() {
    if (!account) return
    try {
      await api.compose.saveDraft({
        accountId: account.id,
        email: buildOutgoing(),
        replaceSource: seed.draftRef ?? null,
      })
      toasts.success(m.mobile_draft_saved())
      onclose()
    } catch (e) {
      toasts.error(m.mobile_draft_save_failed(), e)
    }
  }

  const dirty = $derived(
    body !== (seed.body ?? '') ||
      to !== (seed.to ?? '') ||
      subject !== (seed.subject ?? '') ||
      attachments.length !== (seed.attachments?.length ?? 0),
  )

  function requestClose() {
    if (dirty) showDiscard = true
    else onclose()
  }

  function moreActions(): SheetAction[] {
    return [
      { label: m.mobile_save_draft(), icon: 'save-draft', run: saveDraft },
      { label: m.mobile_attach_file(), icon: 'attachment', run: () => fileInput?.click() },
      {
        label: m.mobile_attach_from_nextcloud(),
        icon: 'cloud',
        run: () => (ncPickerOpen = true),
      },
      {
        label: encrypt ? m.mobile_encryption_off() : m.mobile_encryption_on(),
        icon: encrypt ? 'unlocked' : 'encrypted',
        run: () => (encrypt = !encrypt),
      },
      {
        label: m.mobile_discard(),
        icon: 'trash',
        destructive: true,
        run: () => (showDiscard = true),
      },
    ]
  }
</script>

<div class="compose-shell">
  <div class="screen">
    <NavBar title={m.mobile_new_message()}>
      {#snippet leading()}
        <button type="button" class="bar-button bar-button--text" onclick={requestClose}>
          {m.mobile_cancel()}
        </button>
      {/snippet}
      {#snippet actions()}
        <button
          type="button"
          class="bar-button"
          aria-label={m.mobile_more_actions()}
          onclick={() => (moreOpen = true)}
        >
          <Icon name="more" size={20} />
        </button>
        <button
          type="button"
          class="bar-button bar-button--text bar-button--filled"
          disabled={!canSend}
          onclick={send}
        >
          {sending ? m.mobile_sending() : m.mobile_send()}
        </button>
      {/snippet}
    </NavBar>

    <div class="screen__body form-group">
      <!-- Recipients -->
      <div class="compose-field">
        <span class="compose-field__label">{m.mobile_from()}</span>
        <select bind:value={accountId} class="compose-field__select" aria-label={m.mobile_from()}>
          {#each accounts as acc (acc.id)}
            <option value={acc.id}>
              {acc.person_name || acc.display_name || acc.email} · {acc.email}
            </option>
          {/each}
        </select>
      </div>

      <div class="compose-field">
        <span class="compose-field__label">{m.mobile_to()}</span>
        <input
          type="text"
          inputmode="email"
          autocapitalize="none"
          autocorrect="off"
          spellcheck="false"
          bind:value={to}
          aria-label={m.mobile_to()}
          oninput={() => updateSuggestions('to', to)}
          onfocus={() => (activeField = 'to')}
          class="compose-field__input"
        />
        {#if !showCc}
          <button
            type="button"
            class="value-button"
            onclick={() => (showCc = true)}
            aria-label={m.mobile_show_cc_bcc()}
          >
            <Icon name="plus" size={16} />
          </button>
        {/if}
      </div>

      {#if showCc}
        <div class="compose-field">
          <span class="compose-field__label">{m.mobile_cc()}</span>
          <input
            type="text"
            inputmode="email"
            autocapitalize="none"
            autocorrect="off"
            spellcheck="false"
            bind:value={cc}
            aria-label={m.mobile_cc()}
            oninput={() => updateSuggestions('cc', cc)}
            onfocus={() => (activeField = 'cc')}
            class="compose-field__input"
          />
        </div>
        <div class="compose-field">
          <span class="compose-field__label">{m.mobile_bcc()}</span>
          <input
            type="text"
            inputmode="email"
            autocapitalize="none"
            autocorrect="off"
            spellcheck="false"
            bind:value={bcc}
            aria-label={m.mobile_bcc()}
            oninput={() => updateSuggestions('bcc', bcc)}
            onfocus={() => (activeField = 'bcc')}
            class="compose-field__input"
          />
        </div>
      {/if}

      {#if suggestions.length > 0}
        <ul class="suggestions">
          {#each suggestions as suggestion (suggestion.address)}
            <li>
              <button type="button" class="list-row" onclick={() => applySuggestion(suggestion)}>
                <Icon name="contacts" size={18} />
                <span class="min-w-0 flex-1">
                  <span class="block truncate list-row__title">{suggestion.label}</span>
                  <span class="block truncate list-row__detail">{suggestion.address}</span>
                </span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}

      <div class="compose-field">
        <span class="compose-field__label">{m.mobile_subject()}</span>
        <input
          type="text"
          bind:value={subject}
          class="compose-field__input"
          aria-label={m.mobile_subject()}
        />
      </div>

      {#if encrypt}
        <div class="compose-notice">
          <span class="ink-accent inline-flex"><Icon name="encrypted" size={16} /></span>
          <span class="flex-1">{m.mobile_encryption_active()}</span>
          <button type="button" class="value-button" onclick={() => (encrypt = false)}>
            {m.mobile_turn_off()}
          </button>
        </div>
      {/if}

      <!-- Attachments -->
      {#if attachments.length > 0}
        <div class="page-inset py-2">
          <div class="flex flex-wrap gap-2">
            {#each attachments as att, index (att.filename + index)}
              <span class="attachment-chip">
                <FileTypeIcon
                  filename={att.filename}
                  contentType={att.content_type}
                  class="w-4 h-4"
                />
                <span class="truncate max-w-[9rem]">{att.filename}</span>
                <span class="t-micro ink-muted">{fileSize(att.data.length)}</span>
                <button
                  type="button"
                  aria-label={m.mobile_remove()}
                  onclick={() => removeAttachment(index)}
                >
                  <Icon name="close" size={13} />
                </button>
              </span>
            {/each}
          </div>
          {#if attachmentBytes > 15_000_000}
            <p class="mt-2 text-xs ink-warning">
              {m.mobile_attachment_size_warning({ size: fileSize(attachmentBytes) })}
            </p>
          {/if}
        </div>
      {/if}

      <!-- Body -->
      <textarea
        class="compose-body"
        bind:value={body}
        aria-label={m.mobile_message_placeholder()}
        placeholder={m.mobile_message_placeholder()}
        autocapitalize="sentences"
      ></textarea>

      {#if signature}
        <div class="page-inset pb-2 t-footnote ink-muted">
          <div class="compose-signature">
            {@html signature}
          </div>
        </div>
      {/if}

      {#if seed.quotedHtml}
        <details class="compose-quote">
          <summary class="compose-quote__summary">{m.mobile_quoted_message()}</summary>
          <div class="email-html-body email-html-body--native px-3 pb-3 text-sm">
            {@html seed.quotedHtml}
          </div>
        </details>
      {/if}
    </div>

    <!-- Quick actions above the keyboard -->
    <div class="compose-toolbar">
      <button
        type="button"
        class="bar-button"
        aria-label={m.mobile_attach_file()}
        onclick={() => fileInput?.click()}
      >
        <Icon name="attachment" size={20} />
      </button>
      <button
        type="button"
        class="bar-button"
        aria-label={m.mobile_attach_from_nextcloud()}
        onclick={() => (ncPickerOpen = true)}
      >
        <Icon name="cloud" size={20} />
      </button>
      <button
        type="button"
        class="bar-button"
        aria-label={encrypt ? m.mobile_encryption_off() : m.mobile_encryption_on()}
        onclick={() => (encrypt = !encrypt)}
      >
        <Icon name={encrypt ? 'encrypted' : 'unlocked'} size={20} />
      </button>
      <span class="flex-1"></span>
      <button
        type="button"
        class="bar-button"
        aria-label={m.mobile_save_draft()}
        onclick={saveDraft}
      >
        <Icon name="save-draft" size={20} />
      </button>
    </div>

    <input bind:this={fileInput} type="file" multiple class="hidden" onchange={onPickFiles} />
  </div>
</div>

{#if moreOpen}
  <ActionSheet actions={moreActions()} onclose={() => (moreOpen = false)} />
{/if}

{#if confirmMissingAttachment}
  <Confirm
    title={m.mobile_missing_attachment_title()}
    body={m.mobile_missing_attachment_body()}
    confirmLabel={m.mobile_send_anyway()}
    destructive={false}
    onconfirm={() => {
      confirmMissingAttachment = false
      void send()
    }}
    oncancel={() => {
      confirmMissingAttachment = false
      fileInput?.click()
    }}
  />
{/if}

{#if showDiscard}
  <Confirm
    title={m.mobile_discard_title()}
    body={m.mobile_discard_body()}
    confirmLabel={m.mobile_discard()}
    onconfirm={() => {
      showDiscard = false
      onclose()
    }}
    oncancel={() => (showDiscard = false)}
  />
{/if}

{#if ncPickerOpen}
  <NextcloudPicker
    mode="file"
    onpick={(ncId, path) => attachFromNextcloud(ncId, path)}
    onclose={() => (ncPickerOpen = false)}
  />
{/if}

<style>
  /* The composer covers the whole app, tab bar included — it's a
     mode, not a screen you can wander away from mid-sentence. */
  /* One sheet of paper: the writing surface is the card colour edge to
     edge, and the header fields are lines on it, not boxes. */
  .compose-shell {
    position: fixed;
    inset: 0;
    /* Ends above the keyboard (`lib/keyboardViewport.ts`), so the line
       being typed stays in view and the bar with Send stays on screen. */
    bottom: var(--keyboard-inset, 0px);
    z-index: 55;
    background: var(--ui-card);
    --ui-canvas: var(--ui-card);
  }

  .compose-field {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    min-height: 48px;
    padding: var(--s-1) var(--gutter);
    border-bottom: 1px solid var(--ui-hairline);
  }

  .compose-field__label {
    width: 3.75rem;
    flex-shrink: 0;
    font-size: var(--t-callout);
    color: var(--ui-ink-muted);
  }

  /* Recipient rows read as one line of text, not as boxes: four
     bordered rectangles stacked above the message would dominate the
     screen the moment the keyboard is up. */
  .compose-field__input,
  .compose-field__select {
    flex: 1;
    min-width: 0;
    /* Long addresses would otherwise run under the select's own
       chevron. */
    text-overflow: ellipsis;
    border: 0 !important;
    background: transparent !important;
    padding: 6px 0 !important;
    box-shadow: none !important;
  }

  .suggestions {
    max-height: 40vh;
    overflow-y: auto;
    border-bottom: 1px solid var(--ui-hairline);
  }

  .compose-notice {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    margin: var(--s-2) var(--gutter);
    padding: var(--s-2) var(--s-3);
    border-radius: var(--r-md);
    background: var(--ui-tint-accent);
    font-size: var(--t-footnote);
    color: var(--ui-ink);
  }

  .compose-signature {
    padding-top: var(--s-2);
    border-top: 1px solid var(--ui-hairline);
  }

  .compose-quote {
    margin: 0 var(--gutter) var(--s-6);
    border-radius: var(--r-md);
    border: 1px solid var(--ui-border);
  }

  .compose-quote__summary {
    min-height: 44px;
    padding: var(--s-3);
    font-size: var(--t-callout);
    color: var(--ui-ink-muted);
    cursor: pointer;
  }

  .compose-body {
    width: 100%;
    min-height: 40vh;
    border: 0 !important;
    background: transparent !important;
    padding: var(--s-4) var(--gutter) !important;
    resize: none;
    outline: none;
    box-shadow: none !important;
    line-height: 1.5;
  }

  .attachment-chip {
    display: inline-flex;
    align-items: center;
    gap: var(--s-2);
    padding: var(--s-1) var(--s-3);
    border-radius: var(--r-full);
    font-size: var(--t-caption);
    background: var(--ui-fill-quiet);
  }

  .compose-toolbar {
    display: flex;
    align-items: center;
    gap: var(--s-1);
    padding-inline: var(--s-2);
    /* The home indicator's space — unless the keyboard covers it, when
       the bar sits right on the keyboard (`lib/keyboardViewport.ts`). */
    padding-bottom: max(0px, calc(env(safe-area-inset-bottom) - var(--keyboard-inset, 0px)));
    flex-shrink: 0;
    background: var(--ui-card);
    border-top: 1px solid var(--ui-hairline);
  }
</style>
