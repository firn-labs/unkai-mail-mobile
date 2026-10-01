<script lang="ts">
  /**
   * The reader.
   *
   * The pipeline from stored bytes to something readable on a phone
   * is in `mail/renderHtml.ts`; this screen owns the *decisions*
   * around it: whether remote images may load, when to fetch the
   * message's own inline parts, when to ask for a passphrase, and
   * what the action bar offers.
   *
   * Layout is one scrolling column — header, invite card,
   * attachments, body — with a fixed action bar at the bottom. The
   * desktop's separate toolbar and reading pane collapse into that;
   * the actions stay in thumb reach rather than following the
   * content off-screen.
   */
  import * as api from '../api'
  import type { Email, InlineImagePart } from '../api'
  import Icon from '../Icon.svelte'
  import Avatar from '../Avatar.svelte'
  import FileTypeIcon from '../FileTypeIcon.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import Spinner from '../ui/Spinner.svelte'
  import Sheet from '../ui/Sheet.svelte'
  import ActionSheet, { type SheetAction } from '../ui/ActionSheet.svelte'
  import { nav } from '../nav.svelte'
  import { toasts } from '../toast.svelte'
  import { settingsStore } from '../settingsStore.svelte'
  import { fullDate, displayName, fileSize, bareEmail } from '../format'
  import { parseFromHeader } from '../fromHeader'
  import { processEmailHtml, textToHtml, themeBaseColors } from '../mail/renderHtml'
  import { buildInlineImageUrls, inlineImageBlob } from '../inlineImages'
  import { addTrustedSender, isSenderTrusted } from '../trustedSenders'
  import { parseMailtoUrl, type MailtoParsed } from '../mailtoUrl'
  import { formatError } from '../errors'
  import { INBOX, type FolderLike } from '../mail/folders'
  import { folderLabel } from '../mail/folderNames'
  import { archiveFolderFor, isArchiveFolder } from '../mail/archive'
  import { envelopeKey, mailLists } from '../mail/listBus'
  import { m } from '../../paraglide/messages'

  interface Props {
    accountId: string
    folder: string
    uid: number
    subject?: string
    onreply: (mail: Email, kind: 'reply' | 'reply-all' | 'forward', uid: number) => void
    /** A `mailto:` link tapped in the body, already parsed. */
    onmailto: (fields: MailtoParsed) => void
  }

  let { accountId, folder, uid, subject = '', onreply, onmailto }: Props = $props()

  let email = $state<Email | null>(null)
  let loading = $state(true)
  let error = $state('')
  let showDetails = $state(false)
  let showImages = $state(false)
  let hadBlocked = $state(false)
  let inlineUrls = $state<Record<string, string>>({})
  let inlineLoading = $state(false)
  let actionsOpen = $state(false)
  /** The screen root: what the body sits on in the dark reading mode,
   *  read for the contrast pass. Bound at mount, before any message
   *  arrives — so the body renders once, not again when a body element
   *  binds (which is what binding the body itself used to cost). */
  let readerEl: HTMLDivElement | undefined = $state()
  /** Bumped when the theme flips so the contrast pass re-runs. */
  let themeVersion = $state(0)
  /** Whether the app is drawn dark right now (`data-mode` on <html>). */
  let isDark = $state(document.documentElement.dataset.mode === 'dark')

  /** Passphrase prompt for an encrypted message we couldn't open
   *  with the keychain. */
  let passphrasePrompt = $state<{ value: string; error: string; busy: boolean } | null>(null)

  const from = $derived(parseFromHeader(email?.from ?? ''))
  /**
   * The dark reading mode: the message on the dark page, with light
   * backgrounds darkened and dark text lightened (`renderHtml`). Only in
   * dark mode, and only when chosen (Settings → Appearance, or the ⋯
   * sheet here) — by default a message is shown on white paper, as its
   * sender laid it out. In light mode the paper is always white.
   */
  const darkReading = $derived(isDark && settingsStore.value?.mail_html_white_background === false)
  const whiteCanvas = $derived(!darkReading)
  const autoLoadImages = $derived(settingsStore.value?.auto_load_remote_images ?? false)
  const isEncrypted = $derived(!!email?.protection)
  const attachments = $derived(
    (email?.attachments ?? []).filter(
      // Parts that only exist to carry an inline image are already
      // shown in the body; listing them again as files is noise.
      (a: { content_id?: string | null; content_type: string }) =>
        !(a.content_id && a.content_type.startsWith('image/')),
    ),
  )

  async function load() {
    loading = true
    error = ''
    try {
      const cached = await api.mail.getCachedMessage({ accountId, folder, uid })
      if (cached) {
        email = cached
        loading = false
      }
    } catch {
      // Cache miss is normal for a message that was never opened.
    }
    // A message under a UID never changes on the server — only its
    // flags do, and those come with the list. So a message whose body
    // is already cached is not fetched again: that fetch was a whole
    // IMAP connection and the full message re-downloaded on *every*
    // open (1–2 s on a phone connection), and its answer then rendered
    // the body a second time under the user's finger.
    const haveBody = !!(email?.body_html || email?.body_text)
    if (!haveBody) {
      try {
        email = await api.mail.fetchMessage({ accountId, folder, uid })
        error = ''
      } catch (e) {
        if (!email) error = formatError(e)
      }
    }
    loading = false

    if (email) {
      showImages = autoLoadImages || isSenderTrusted(email.from)
      void markRead()
      void loadInlineImages()
    }
  }

  async function markRead() {
    if (email?.is_read) return
    try {
      await api.mail.markAsRead({ accountId, folder, uid })
    } catch {
      // Not worth a toast: the message is open and readable either
      // way, and the next sync reconciles the flag.
    }
  }

  /**
   * Fetch the message's own `cid:` image parts.
   *
   * Gated on the body actually referencing one — the overwhelming
   * majority of mail has no inline images, and this would otherwise
   * be a wasted IMAP round-trip per message opened.
   */
  async function loadInlineImages() {
    const html = email?.body_html
    if (!html || !/\bcid:/i.test(html)) return
    inlineLoading = true
    try {
      const parts: InlineImagePart[] = await api.mail.fetchInlineImages({
        accountId,
        folder,
        uid,
        ...(isEncrypted ? { pgpPassphrase: '' } : {}),
      })
      // Object URLs rather than data: URIs — the bytes stay out of
      // the DOM, and the reader revokes them when it unmounts.
      inlineUrls = buildInlineImageUrls(parts, (part) =>
        URL.createObjectURL(inlineImageBlob(part)),
      )
    } catch (e) {
      console.warn('inline image fetch failed', e)
    } finally {
      inlineLoading = false
    }
  }

  $effect(() => {
    void accountId
    void folder
    void uid
    void load()
    return () => {
      // Object URLs live as long as this screen does.
      for (const url of Object.values(inlineUrls)) URL.revokeObjectURL(url)
    }
  })

  $effect(() => {
    // Re-run the contrast pass when the OS or the user flips the
    // theme with a message open.
    const observer = new MutationObserver(() => {
      isDark = document.documentElement.dataset.mode === 'dark'
      themeVersion += 1
    })
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['data-theme', 'data-mode'],
    })
    return () => observer.disconnect()
  })

  /**
   * The body as one string. A derived *string*, so that a new `email`
   * object carrying the same body (a flag toggled, the message marked
   * read) does not run the whole pipeline — sanitise, parse, fold,
   * re-tint — and replace the rendered DOM again.
   */
  const bodySource = $derived(
    email?.body_html ? email.body_html : email?.body_text ? textToHtml(email.body_text) : '',
  )

  const rendered = $derived.by(() => {
    void themeVersion
    const source = bodySource
    if (!source) return { html: '', hadBlocked: false }
    return processEmailHtml(source, {
      showImages,
      inlineUrls,
      inlineLoading,
      // On white paper the sender's own assumptions hold, so their
      // colours are left exactly as written.
      adjustContrast: darkReading,
      darkReading,
      contrastColors: darkReading && readerEl ? themeBaseColors(readerEl) : undefined,
    })
  })

  $effect(() => {
    hadBlocked = rendered.hadBlocked
  })

  /**
   * Body taps: links leave the app, `cid:` anchors open the part
   * they point at, everything else falls through to selection.
   *
   * One delegated listener rather than rewriting hrefs, so the
   * sanitised DOM stays a faithful copy of what the sender wrote.
   */
  function onBodyClick(e: MouseEvent) {
    const anchor = (e.target as HTMLElement | null)?.closest('a')
    if (!anchor) return
    e.preventDefault()
    const cid = anchor.getAttribute('data-unkai-cid')
    if (cid) {
      const url = inlineUrls[cid]
      if (url) window.open(url, '_blank')
      return
    }
    const href = anchor.getAttribute('href') ?? ''
    if (!href || href === '#') return
    if (href.toLowerCase().startsWith('mailto:')) {
      openMailto(href)
      return
    }
    void api.system.openUrl({ url: href }).catch((err) => toasts.error(m.mobile_open_failed(), err))
  }

  function openMailto(href: string) {
    // RFC 6068 allows subject / body / cc / bcc in the URL, and
    // plenty of "email us" links use them. Parse the whole thing and
    // hand the fields up — the composer is a shell-level modal.
    onmailto(parseMailtoUrl(href))
  }

  function trustSender() {
    if (!email) return
    addTrustedSender(email.from)
    showImages = true
    toasts.success(m.mobile_images_always_shown({ sender: from.email || email.from }))
  }

  /* ── Decryption ──────────────────────────────────────────────── */

  async function decryptWithPassphrase() {
    if (!passphrasePrompt || !email) return
    // Hold the entered value locally: the prompt object is replaced
    // below, and reading it back after an await would race the user
    // closing the sheet.
    const entered = passphrasePrompt.value
    passphrasePrompt = { value: entered, busy: true, error: '' }
    try {
      const decrypted = await api.crypto.decryptMessage({
        accountId,
        folder,
        uid,
        pgpPassphrase: entered,
      })
      email = { ...email, ...decrypted }
      passphrasePrompt = null
      void loadInlineImages()
    } catch (e) {
      passphrasePrompt = {
        value: entered,
        busy: false,
        // `formatError` unwraps the tagged variant; what's left is
        // the sentence the user needs.
        error: formatError(e),
      }
    }
  }

  async function tryAutoDecrypt() {
    if (!email) return
    try {
      const auto = await api.crypto.tryAutoDecryptMessage({ accountId, folder, uid })
      if (auto) {
        email = { ...email, ...auto }
        void loadInlineImages()
        return
      }
    } catch {
      // Fall through to the prompt — the account opted into
      // automatic unlock but the key no longer opens it.
    }
    passphrasePrompt = { value: '', error: '', busy: false }
  }

  /* ── Attachments ─────────────────────────────────────────────── */

  let busyAttachment = $state<number | null>(null)

  async function openAttachment(att: {
    part_id: number
    filename: string
    content_type: string
  }) {
    busyAttachment = att.part_id
    try {
      const bytes = isEncrypted
        ? await api.crypto.downloadDecryptedAttachment({
            accountId,
            folder,
            uid,
            partId: att.part_id,
            pgpPassphrase: '',
          })
        : await api.mail.downloadEmailAttachment({
            accountId,
            folder,
            uid,
            partId: att.part_id,
          })
      const blob = new Blob([new Uint8Array(bytes)], { type: att.content_type })
      const url = URL.createObjectURL(blob)
      // The webview renders images, PDFs and text itself; anything
      // else it can't render simply doesn't open, which is why the
      // Save action below exists next to this one.
      window.open(url, '_blank')
      setTimeout(() => URL.revokeObjectURL(url), 60_000)
    } catch (e) {
      toasts.error(m.mobile_attachment_open_failed(), e)
    } finally {
      busyAttachment = null
    }
  }

  async function saveAttachment(att: { part_id: number; filename: string }) {
    busyAttachment = att.part_id
    try {
      const path = await api.system.saveAttachmentToDocuments({
        accountId,
        folder,
        uid,
        partId: att.part_id,
        filename: att.filename,
      })
      toasts.success(m.mobile_attachment_saved({ name: path.split('/').pop() ?? att.filename }))
    } catch (e) {
      toasts.error(m.mobile_attachment_save_failed(), e)
    } finally {
      busyAttachment = null
    }
  }

  /* ── Message actions ─────────────────────────────────────────── */

  /** Whether this message already sits in its account's archive, where
   *  Archive would be a no-op — see `mail/archive.ts`. */
  let inArchive = $state(false)

  $effect(() => {
    const target = { accountId, folder }
    void archiveFolderFor(target.accountId).then((archiveName) => {
      inArchive = isArchiveFolder(target.folder, archiveName)
    })
  })

  /** This message's row in the lists underneath (`mail/listBus`). */
  const rowKey = $derived(envelopeKey({ account_id: accountId, folder, uid }))

  /**
   * Take the message away, go back at once, and let the server catch up.
   *
   * Archive, Delete and Move used to wait for the server before going
   * back — a connection, a login and a MOVE, 1–3 seconds on a phone
   * during which the button seemed to do nothing. Now the list drops
   * the row and comes back immediately; the toast confirms when the
   * server has done it, and a refusal puts the row back and says why.
   */
  function leaveWith(
    run: () => Promise<unknown>,
    done: string,
    failed: string,
  ): void {
    const key = rowKey
    mailLists.hide(key)
    nav.pop()
    run().then(
      () => {
        mailLists.settle(key)
        toasts.show(done)
      },
      (e) => {
        mailLists.unhide(key)
        toasts.error(failed, e)
      },
    )
  }

  function unarchive() {
    leaveWith(
      () => api.mail.moveMessage({ accountId, folder, uid, destFolder: INBOX }),
      m.mobile_unarchived(),
      m.mobile_move_failed(),
    )
  }

  function archive() {
    leaveWith(
      () => api.mail.archiveMessage({ accountId, folder, uid }),
      m.mobile_archived(),
      m.mobile_archive_failed(),
    )
  }

  function trash() {
    leaveWith(
      () => api.mail.deleteMessage({ accountId, folder, uid }),
      m.mobile_moved_to_trash(),
      m.mobile_delete_failed(),
    )
  }

  /** The flag shows its new state at once; a refusal turns it back. */
  async function toggleFlag() {
    if (!email) return
    const flagged = !email.is_starred
    const key = rowKey
    email = { ...email, is_starred: flagged }
    mailLists.patch(key, { is_starred: flagged })
    try {
      await api.mail.setMessageFlagged({ accountId, folder, uid, flagged })
    } catch (e) {
      if (email) email = { ...email, is_starred: !flagged }
      mailLists.patch(key, { is_starred: !flagged })
      toasts.error(m.mobile_flag_update_failed(), e)
    }
  }

  function markUnreadAndLeave() {
    const key = rowKey
    mailLists.patch(key, { is_read: false })
    nav.pop()
    api.mail.setMessageRead({ accountId, folder, uid, read: false }).catch((e) => {
      mailLists.patch(key, { is_read: true })
      toasts.error(m.mobile_flag_update_failed(), e)
    })
  }

  /** Flip the dark reading mode from here — the same setting as in
   *  Settings → Appearance, so the choice sticks for the next message. */
  async function toggleReadingMode() {
    try {
      await settingsStore.patch({ mail_html_white_background: darkReading })
    } catch (e) {
      toasts.error(m.mobile_settings_save_failed(), e)
    }
  }

  function moreActions(): SheetAction[] {
    if (!email) return []
    return [
      // Reply-all lives here rather than in the bar: the bar keeps one
      // clear primary action, and reply-all is the one that is easy to
      // send by mistake.
      { label: m.mobile_reply_all(), icon: 'reply-all', run: () => onreply(email!, 'reply-all', uid) },
      { label: m.mobile_mark_unread(), icon: 'unread', run: markUnreadAndLeave },
      {
        label: email.is_starred ? m.mobile_unflag() : m.mobile_flag(),
        icon: 'flag',
        run: toggleFlag,
      },
      { label: m.mobile_move_to_folder(), icon: 'move-to-folder', run: () => (moveOpen = true) },
      // Only where it does something: the reading mode applies in dark
      // mode alone, so in light mode the row would be a switch that
      // visibly changes nothing.
      ...(isDark
        ? [
            darkReading
              ? { label: m.mobile_reading_show_light(), icon: 'sun' as const, run: toggleReadingMode }
              : { label: m.mobile_reading_show_dark(), icon: 'moon' as const, run: toggleReadingMode },
          ]
        : []),
      ...(hadBlocked && !showImages
        ? [
            {
              label: m.mobile_always_show_images(),
              icon: 'insert-image' as const,
              run: trustSender,
            },
          ]
        : []),
      ...(isEncrypted
        ? [{ label: m.mobile_decrypt(), icon: 'unlocked' as const, run: tryAutoDecrypt }]
        : []),
      {
        label: m.mobile_delete(),
        icon: 'trash',
        destructive: true,
        run: trash,
      },
    ]
  }

  let moveOpen = $state(false)
  /** Destinations with the name to show (`folderLabel`). */
  let moveFolders = $state<{ name: string; label: string }[]>([])

  $effect(() => {
    if (!moveOpen) return
    void (async () => {
      try {
        const list: FolderLike[] = await api.mail.getCachedFolders({ accountId })
        moveFolders = list
          .filter((f) => f.name !== folder)
          .map((f) => ({ name: f.name, label: folderLabel(f) }))
      } catch (e) {
        toasts.error(m.mobile_folders_load_failed(), e)
        moveOpen = false
      }
    })()
  })

  function moveTo(dest: { name: string; label: string }) {
    leaveWith(
      () => api.mail.moveMessage({ accountId, folder, uid, destFolder: dest.name }),
      m.mobile_moved_to({ folder: dest.label }),
      m.mobile_move_failed(),
    )
  }
</script>

<!-- A plain screen: the message is set on the page like a letter, not
     boxed into cards (v3). -->
<div class="screen screen--plain reader" bind:this={readerEl}>
  <!-- No bar title: the subject is the first thing in the content, and
       stating it twice was the first thing the old reader did. -->
  <NavBar title="" backLabel={m.mobile_back()} onback={() => nav.pop()}>
    {#snippet actions()}
      <!-- The flag shows its state: flagged is the warning ink, the
           same colour as the flag glyph in the list. `aria-pressed`
           says it to VoiceOver. -->
      <button
        type="button"
        class="bar-button"
        class:bar-button--flagged={email?.is_starred}
        aria-label={m.mobile_flag()}
        aria-pressed={email?.is_starred ?? false}
        onclick={toggleFlag}
      >
        <Icon name="flag" size={20} />
      </button>
      <button
        type="button"
        class="bar-button"
        aria-label={m.mobile_more_actions()}
        onclick={() => (actionsOpen = true)}
      >
        <Icon name="more" size={20} />
      </button>
    {/snippet}
  </NavBar>

  {#if loading && !email}
    <Spinner label={m.mobile_loading()} />
  {:else if !email}
    <div class="flex-1 flex items-center justify-center p-6 text-center">
      <p class="text-sm ink-danger">{error || m.mobile_message_load_failed()}</p>
    </div>
  {:else}
    <div class="screen__body">
      <!-- Header -->
      <div class="reader-head">
        <h1 class="reader-head__subject selectable">
          {email.subject || m.mobile_no_subject()}
        </h1>

        <!-- The sender gets the whole line: the date used to share it
             and cut the name to "Jamie Fis…". Recipients sit under it and
             the full addresses open below — a letterhead, not a card. -->
        <button
          type="button"
          class="sender"
          aria-expanded={showDetails}
          onclick={() => (showDetails = !showDetails)}
        >
          <Avatar
            displayName={from.name || from.email || email.from}
            size={40}
          />
          <span class="sender__text">
            <span class="sender__line">
              <span class="sender__name">{from.name || from.email}</span>
              <span class="sender__date">{fullDate(email.date)}</span>
            </span>
            <span class="sender__meta">
              {m.mobile_to_line({ recipients: email.to.map(displayName).join(', ') || '—' })}
              <span class="sender__chevron" class:sender__chevron--open={showDetails} aria-hidden="true">
                <Icon name="nav-forward" size={12} />
              </span>
            </span>
          </span>
        </button>

        {#if showDetails}
          <dl class="details selectable">
            <div class="flex gap-2 py-0.5">
              <dt class="w-12 shrink-0 ink-muted">{m.mobile_from()}</dt>
              <dd class="min-w-0 break-all">{email.from}</dd>
            </div>
            <div class="flex gap-2 py-0.5">
              <dt class="w-12 shrink-0 ink-muted">{m.mobile_to()}</dt>
              <dd class="min-w-0 break-all">{email.to.join(', ') || '—'}</dd>
            </div>
            {#if email.cc.length > 0}
              <div class="flex gap-2 py-0.5">
                <dt class="w-12 shrink-0 ink-muted">{m.mobile_cc()}</dt>
                <dd class="min-w-0 break-all">{email.cc.join(', ')}</dd>
              </div>
            {/if}
            <div class="flex gap-2 py-0.5">
              <dt class="w-12 shrink-0 ink-muted">{m.mobile_date()}</dt>
              <dd>{fullDate(email.date)}</dd>
            </div>
          </dl>
        {/if}

        {#if email.protection || email.signature_status}
          <div class="mt-3 flex flex-wrap gap-1.5">
            {#if email.protection}
              <span class="chip chip--ok">
                <Icon name="encrypted" size={13} />
                {m.mobile_encrypted()}
              </span>
            {/if}
            {#if email.signature_status === 'valid'}
              <span class="chip chip--ok">
                <Icon name="verified" size={13} />
                {m.mobile_signature_valid()}
              </span>
            {:else if email.signature_status}
              <span class="chip chip--warn">
                <Icon name="warning" size={13} />
                {m.mobile_signature_unverified()}
              </span>
            {/if}
          </div>
        {/if}
      </div>

      <!-- Encrypted but still sealed -->
      {#if isEncrypted && !email.body_html && !email.body_text}
        <div class="notice notice--center">
          <Icon name="lock" size={22} />
          <p class="mt-1 text-sm">{m.mobile_encrypted_body()}</p>
          <button type="button" class="wide-button mt-3" onclick={tryAutoDecrypt}>
            {m.mobile_decrypt()}
          </button>
        </div>
      {/if}

      <!-- Remote images withheld -->
      {#if hadBlocked && !showImages}
        <div class="notice">
          <span class="notice__icon"><Icon name="shield-image-blocked" size={20} /></span>
          <span class="min-w-0 flex-1 t-footnote">{m.mobile_images_blocked()}</span>
          <button type="button" class="value-button" onclick={() => (showImages = true)}>
            {m.mobile_show()}
          </button>
        </div>
      {/if}

      <!-- Attachments -->
      {#if attachments.length > 0}
        <div class="attachments">
          {#each attachments as att (att.part_id)}
            <div class="attachment">
              <button
                type="button"
                class="attachment__open"
                onclick={() => openAttachment(att)}
                disabled={busyAttachment === att.part_id}
              >
                {#if busyAttachment === att.part_id}
                  <Icon name="loading" size={20} />
                {:else}
                  <FileTypeIcon filename={att.filename} contentType={att.content_type} class="w-5 h-5" />
                {/if}
                <span class="attachment__name">{att.filename}</span>
                <span class="attachment__size">{fileSize(att.size)}</span>
              </button>
              <button
                type="button"
                class="attachment__save"
                aria-label={m.mobile_save()}
                onclick={() => saveAttachment(att)}
              >
                <Icon name="download" size={16} />
              </button>
            </div>
          {/each}
        </div>
      {/if}

      <!-- Body -->
      <div
        class="email-html-body reader-body pb-8"
        class:email-html-body--white={whiteCanvas}
        class:email-html-body--native={!whiteCanvas}
        onclick={onBodyClick}
        role="presentation"
      >
        {@html rendered.html}
      </div>
      <div class="reader-end"></div>
    </div>

    <!-- Action bar: the triage actions on the left, Reply as the one
         filled button on the right, where the thumb rests. Reply-all is
         in the ⋯ sheet (see `moreActions`). -->
    <div class="action-bar">
      {#if inArchive}
        <button type="button" class="action-bar__btn" onclick={unarchive}>
          <Icon name="global-inbox" size={20} />
          <span>{m.mobile_unarchive_short()}</span>
        </button>
      {:else}
        <button type="button" class="action-bar__btn" onclick={archive}>
          <Icon name="archive" size={20} />
          <span>{m.mobile_archive()}</span>
        </button>
      {/if}
      <button type="button" class="action-bar__btn action-bar__btn--danger" onclick={trash}>
        <Icon name="trash" size={20} />
        <span>{m.mobile_delete()}</span>
      </button>
      <button type="button" class="action-bar__btn" onclick={() => onreply(email!, 'forward', uid)}>
        <Icon name="forward" size={20} />
        <span>{m.mobile_forward()}</span>
      </button>
      <button type="button" class="action-bar__reply" onclick={() => onreply(email!, 'reply', uid)}>
        <Icon name="reply" size={20} />
        <span>{m.mobile_reply()}</span>
      </button>
    </div>
  {/if}
</div>

{#if actionsOpen}
  <ActionSheet
    title={email?.subject ?? ''}
    actions={moreActions()}
    onclose={() => (actionsOpen = false)}
  />
{/if}

{#if moveOpen}
  <ActionSheet
    title={m.mobile_move_to_folder()}
    actions={moveFolders.map((dest) => ({
      label: dest.label,
      icon: 'move-to-folder' as const,
      run: () => moveTo(dest),
    }))}
    onclose={() => (moveOpen = false)}
  />
{/if}

{#if passphrasePrompt}
  <Sheet label={m.mobile_passphrase_title()} onclose={() => (passphrasePrompt = null)}>
    <div class="form-group px-4 pb-5 pt-1">
      <h2 class="text-base font-semibold">{m.mobile_passphrase_title()}</h2>
      <p class="mt-1 text-sm ink-muted">{m.mobile_passphrase_body()}</p>
      <input
        type="password"
        class="mt-3"
        autocomplete="current-password"
        aria-label={m.mobile_passphrase_title()}
        bind:value={passphrasePrompt.value}
        onkeydown={(e) => e.key === 'Enter' && decryptWithPassphrase()}
      />
      {#if passphrasePrompt.error}
        <p class="mt-2 text-xs ink-danger">{passphrasePrompt.error}</p>
      {/if}
      <button
        type="button"
        class="wide-button mt-4"
        disabled={passphrasePrompt.busy || !passphrasePrompt.value}
        onclick={decryptWithPassphrase}
      >
        {passphrasePrompt.busy ? m.mobile_working() : m.mobile_decrypt()}
      </button>
    </div>
  </Sheet>
{/if}

<style>
  /* Flagged: the warning ink *and* a tinted pad behind the glyph. The
     pad is the shape cue, so the state does not rest on telling the
     flag's two colours apart (WCAG 1.4.1). */
  .bar-button--flagged {
    color: var(--ui-flagged);
    background: var(--ui-tint-warning);
    border-radius: var(--r-full);
  }

  /* Light mode: the letter is on white paper. The plain page is a
     near-white (`surface-50`), and the "always white" body drew a
     visible band across it; the page takes the white instead. White is
     one of the backgrounds every ink is solved against. Dark mode keeps
     the night page, with the white body as a sheet laid on it. */
  :global(html:not([data-mode='dark'])) .reader {
    --ui-canvas: var(--ui-card-raised);
  }

  .reader-head {
    padding: var(--s-1) var(--gutter) var(--s-4);
  }

  /* The subject is the message's title: the display face, a step below
     a screen title, so it reads as *this* letter's heading rather than
     as a place in the app. */
  .reader-head__subject {
    font-family: var(--font-display);
    font-size: var(--t-title);
    line-height: var(--t-title-lh);
    font-weight: 750;
    letter-spacing: -0.02em;
    color: var(--ui-ink);
    /* `pretty`, not `balance`: balancing a two-line subject pushed the
       dash of "Re: Figures — attached" to the start of line two. */
    text-wrap: pretty;
      overflow-wrap: break-word;
    -webkit-hyphens: auto;
    hyphens: auto;
  }

  /* The letterhead: who, when, to whom. On the page, closed off from
     the body by a hairline rather than a box — v2 set it in a card of
     its own, one of four cards on a screen that shows one letter. */
  .sender {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    width: 100%;
    margin-top: var(--s-4);
    padding-block: var(--s-3);
    border-top: 1px solid var(--ui-hairline);
    border-bottom: 1px solid var(--ui-hairline);
    text-align: left;
    transition: background-color var(--m-fast) var(--m-ease);
  }

  .sender:active {
    background: var(--ui-press);
  }

  .sender__text {
    display: block;
    flex: 1;
    min-width: 0;
  }

  .sender__line {
    display: flex;
    align-items: baseline;
    gap: var(--s-2);
  }

  .sender__name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--t-headline);
    line-height: var(--t-headline-lh);
    font-weight: 650;
    color: var(--ui-ink);
  }

  .sender__date {
    flex-shrink: 0;
    font-size: var(--t-footnote);
    line-height: var(--t-footnote-lh);
    font-variant-numeric: tabular-nums;
    color: var(--ui-ink-faint);
  }

  .sender__meta {
    display: flex;
    align-items: center;
    gap: var(--s-1);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--t-footnote);
    line-height: var(--t-footnote-lh);
    color: var(--ui-ink-muted);
  }

  .sender__chevron {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--ui-ink-faint);
    transform: rotate(90deg);
    transition: transform var(--m-base) var(--m-ease);
  }

  .sender__chevron--open {
    transform: rotate(-90deg);
  }

  .details {
    margin-top: var(--s-3);
    padding: var(--s-3);
    border-radius: var(--r-md);
    background: var(--ui-fill-quiet);
    font-size: var(--t-footnote);
    line-height: var(--t-footnote-lh);
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: var(--s-1);
    padding: 2px var(--s-2);
    border-radius: var(--r-full);
    font-size: var(--t-micro);
    font-weight: 600;
    /* The label is the primary ink; the tint and the icon carry the
       status. Coloured text on its own tint was the weakest pairing
       on the screen — a 700 shade on a pale theme's tint fell under AA. */
    color: var(--ui-ink);
  }
  .chip--ok {
    background: var(--ui-tint-success);
  }
  .chip--warn {
    background: var(--ui-tint-warning);
  }

  /* An inline notice above the body: images withheld, still encrypted. */
  .notice {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    margin: 0 var(--gutter) var(--s-3);
    padding: var(--s-3);
    border-radius: var(--r-md);
    background: var(--ui-tint-accent);
    color: var(--ui-ink);
  }

  .notice--center {
    flex-direction: column;
    padding: var(--s-4);
    text-align: center;
  }

  .notice__icon {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--ui-accent-ink);
  }

  .attachments {
    display: flex;
    gap: var(--s-2);
    margin: 0 var(--gutter) var(--s-4);
    padding-bottom: var(--s-1);
    overflow-x: auto;
  }

  /* A file is a quiet object on the page: the grey fill, no outline. */
  .attachment {
    display: flex;
    align-items: stretch;
    flex-shrink: 0;
    max-width: 260px;
    border-radius: var(--r-md);
    background: var(--ui-fill-quiet);
    overflow: hidden;
  }
  .attachment__open {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    min-height: 44px;
    padding: var(--s-2) var(--s-3);
    min-width: 0;
  }
  .attachment__open:active {
    background: var(--ui-press);
  }
  .attachment__name {
    font-size: var(--t-footnote);
    font-weight: 550;
    max-width: 140px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .attachment__size {
    font-size: var(--t-micro);
    color: var(--ui-ink-muted);
    flex-shrink: 0;
  }
  .attachment__save {
    display: flex;
    align-items: center;
    min-width: 44px;
    justify-content: center;
    border-left: 1px solid var(--ui-hairline);
    color: var(--ui-accent-ink);
  }

  /* The reader's toolbar floats like the tab bar it replaces (the tab
     bar is hidden here): the same capsule, the same place above the home
     indicator, so the thumb finds the controls where it always does.
     Triage on the left, Reply as the one filled button on the right. */
  .action-bar {
    position: absolute;
    left: var(--s-3);
    right: var(--s-3);
    bottom: max(var(--s-2), calc(env(safe-area-inset-bottom) - var(--s-3)));
    z-index: 15;
    display: flex;
    align-items: center;
    gap: var(--s-1);
    padding: var(--s-1);
    border-radius: var(--r-full);
    background: var(--ui-bar);
    box-shadow: var(--e-float);
  }
  .action-bar__btn {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 1px;
    min-height: 52px;
    border-radius: var(--r-full);
    font-size: var(--t-bar-label);
    font-weight: 600;
    color: var(--ui-accent-ink);
  }
  .action-bar__btn span {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .action-bar__btn:active {
    background: var(--ui-press);
  }
  .action-bar__btn--danger {
    color: var(--ui-danger-ink);
  }
  /* The one filled control in the bar. */
  .action-bar__reply {
    display: inline-flex;
    align-items: center;
    gap: var(--s-2);
    flex-shrink: 0;
    min-height: 52px;
    padding: 0 var(--s-5) 0 var(--s-4);
    border-radius: var(--r-full);
    background: var(--ui-accent-fill);
    color: var(--ui-on-accent);
    font-size: var(--t-bar-action);
    font-weight: 650;
    transition: transform var(--m-fast) var(--m-ease);
  }
  .action-bar__reply:active {
    transform: scale(0.96);
  }

  /* The body scrolls clear of the floating toolbar. */
  .reader-end {
    height: calc(var(--s-12) + 52px + env(safe-area-inset-bottom));
  }
</style>
