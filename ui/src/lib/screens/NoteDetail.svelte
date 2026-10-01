<script lang="ts">
  /**
   * The note editor: a plain Markdown textarea with a preview
   * toggle.
   *
   * Notes in Nextcloud *are* Markdown files, so editing the source
   * is editing the note — no round-trip through a rich-text model
   * that would rewrite the user's formatting. The preview renders
   * with `marked` and is sanitised the same way mail bodies are,
   * since a note can contain anything.
   */
  import * as api from '../api'
  import type { Note } from '../api'
  import DOMPurify from 'dompurify'
  import { marked } from 'marked'
  import Icon from '../Icon.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import { nav } from '../nav.svelte'
  import { toasts } from '../toast.svelte'
  import { m } from '../../paraglide/messages'

  interface Props {
    note: Note | null
    ncId: string
  }

  let { note, ncId }: Props = $props()

  /* Read once, deliberately: the note editor is mounted fresh
     for each open and unmounted on close, so the form owns its
     fields from here on. */
  // svelte-ignore state_referenced_locally
  const seed = note

  let title = $state(seed?.title ?? '')
  let content = $state(seed?.content ?? '')
  let category = $state(seed?.category ?? '')
  let preview = $state(false)
  let saving = $state(false)
  let current = $state<Note | null>(seed)

  const dirty = $derived(
    title !== (current?.title ?? '') ||
      content !== (current?.content ?? '') ||
      category !== (current?.category ?? ''),
  )

  const previewHtml = $derived(
    DOMPurify.sanitize(marked.parse(content, { async: false }) as string),
  )

  async function save() {
    if (!title.trim() && !content.trim()) {
      nav.pop()
      return
    }
    saving = true
    try {
      if (current) {
        current = await api.notes.updateNextcloudNote({
          ncId,
          noteId: current.id,
          etag: current.etag,
          title: title.trim() || m.mobile_untitled_note(),
          content,
          category,
        })
      } else {
        current = await api.notes.createNextcloudNote({
          ncId,
          title: title.trim() || m.mobile_untitled_note(),
          content,
          category,
        })
      }
      toasts.success(m.mobile_note_saved())
      nav.pop()
    } catch (e) {
      toasts.error(m.mobile_note_save_failed(), e)
    } finally {
      saving = false
    }
  }
</script>

<div class="screen">
  <NavBar
    title={current ? m.mobile_note() : m.mobile_new_note()}
    backLabel={m.mobile_notes()}
    onback={() => nav.pop()}
  >
    {#snippet actions()}
      <button
        type="button"
        class="bar-button"
        aria-label={preview ? m.mobile_edit() : m.mobile_preview()}
        onclick={() => (preview = !preview)}
      >
        <Icon name={preview ? 'compose' : 'read'} size={20} />
      </button>
      <button
        type="button"
        class="bar-button bar-button--text bar-button--strong"
        disabled={saving || !dirty}
        onclick={save}
      >
        {saving ? m.mobile_saving() : m.mobile_save()}
      </button>
    {/snippet}
  </NavBar>

  <div class="screen__body form-group">
    <div class="page-inset pt-3">
      <input
        type="text"
        bind:value={title}
        aria-label={m.mobile_note_title_placeholder()}
        placeholder={m.mobile_note_title_placeholder()}
      />
      <input
        type="text"
        class="mt-2"
        bind:value={category}
        aria-label={m.mobile_note_category_placeholder()}
        placeholder={m.mobile_note_category_placeholder()}
      />
    </div>

    {#if preview}
      <div class="email-html-body email-html-body--native px-4 py-4 text-sm">
        {@html previewHtml}
      </div>
    {:else}
      <textarea
        class="note-body"
        bind:value={content}
        aria-label={m.mobile_note_body_placeholder()}
        placeholder={m.mobile_note_body_placeholder()}
      ></textarea>
    {/if}
  </div>
</div>

<style>
  .note-body {
    width: 100%;
    min-height: 55vh;
    border: 0 !important;
    background: transparent !important;
    padding: 12px 16px !important;
    resize: none;
    outline: none;
    box-shadow: none !important;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: var(--t-input);
    line-height: 1.5;
  }
</style>
