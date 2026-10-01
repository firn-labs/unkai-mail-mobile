/**
 * Placeholder aliases for the backend DTOs referenced by the generated
 * command wrappers (#473).
 *
 * Every alias is `any` for now: the refactor's first goal is compile-
 * checked command names and argument keys, not full payload typing.
 * Tighten these incrementally — replace an alias with a real interface
 * whenever you touch code that consumes it, the same lazy-migration
 * rule the i18n catalogue uses.
 */

/**
 * Backend `unkai_core::models::Account` (typed for real in #534 —
 * the accountsStore extraction touches every consumer). Keys are
 * snake_case: the Rust struct has no serde rename. Fields the Rust
 * side marks `#[serde(default)]` are optional here so setup-wizard
 * call sites can submit partial objects; on the way *out* of the
 * backend they are always present (`Option` fields as `null`).
 */
export interface Account {
  id: string
  display_name: string
  email: string
  imap_host: string
  imap_port: number
  smtp_host: string
  smtp_port: number
  use_jmap?: boolean
  jmap_url?: string | null
  /** Rich-HTML signature (#248); legacy plain text still occurs. */
  signature?: string | null
  /** "Folder name contains X → icon Y" rules for the Sidebar. */
  folder_icons?: FolderIconRule[]
  /** Per-folder icon overrides, full folder path → emoji. */
  folder_icon_overrides?: Record<string, string>
  /** Full paths of folders hidden from the mailbox list. A display
   *  filter only — hidden folders still sync and still search. */
  hidden_folders?: string[]
  /** User-trusted TLS leaf certs for this account's servers. */
  trusted_certs?: TrustedCert[]
  /** Optional emoji avatar, shown on the account row. */
  emoji?: string | null
  /** Display order in account lists; lower = first. */
  sort_order?: number
  /** Human's full name for the From: header (#115). */
  person_name?: string | null
  /** Display-only hint that a PGP key is imported (#57). */
  pgp_key_fingerprint?: string | null
  /** Display-only hint that an S/MIME identity is imported (#338). */
  smime_cert_fingerprint?: string | null
  /** A demo account (test mode): fixtures in the local cache, no server
   *  behind it. Reads all work; sending is refused. See
   *  `crates/unkai-commands/src/demo.rs`. */
  demo?: boolean
}
/** One entry of `Account.folder_icons`. */
export interface FolderIconRule {
  keyword: string
  icon: string
}
export type AddressbookSummary = any
export type AppSettings = any

/**
 * Backend `unkai_core::models::MobileFeatures` — the mobile-only
 * slice of `AppSettings` that says which features the UI shows and
 * which of them sit on the tab bar (#413 follow-up).
 *
 * Typed for real (unlike `AppSettings` around it) because it is
 * written from one place, `features.svelte.ts`, and a typo in a
 * field name there would silently drop the user's whole layout on
 * the next save.
 */
export interface MobileFeatures {
  /** Feature ids switched off entirely. */
  hidden: string[]
  /**
   * The tab-bar slots between Mail and More, in order. `null` means
   * "never configured" and the UI falls back to its default; `[]` is
   * the deliberate "no middle tabs".
   */
  tabs: string[] | null
}
export type AttachmentPreviewView = any
export type AttendeeAvailability = any
export type CalendarEvent = any
export type CalendarEventInput = any
export type CalendarSummary = any
export type Contact = any
export type ContactCategoryView = any
export type ContactGroupView = any
export type ContactInput = any
export type ContactPhoto = any
export type DatabaseStatusView = any
export type DiscoveredAccount = any
export type DraftReplaceSource = any
export type Email = any
export type EmailEnvelope = any
export type FidoStatusView = any
export type FileEntry = any
export type Folder = any
export type GeocodeResult = any
/**
 * Backend `ImportCalendarReport` (#518) — summary of an `.ics` file
 * import. Typed for real because the import dialog renders every
 * field. Keys are snake_case: the Rust struct has no serde rename.
 */
export interface ImportCalendarReport {
  /** VEVENTs found in the file before dedup / write attempts. */
  total: number
  imported: number
  skipped_duplicates: number
  /** Per-entry failure reasons (recurrence exceptions, write errors). */
  errors: string[]
}
/**
 * Backend `ImportContactsReport` (#484) — summary of a contact file
 * import. Typed for real because the import dialog renders every
 * field. Keys are snake_case: the Rust struct has no serde rename.
 */
export interface ImportContactsReport {
  /** Entries found in the file before dedup / write attempts. */
  total: number
  imported: number
  skipped_duplicates: number
  /** Per-entry failure reasons (unusable rows, write errors). */
  errors: string[]
}
/**
 * Backend `InlineImageView` (#471) — one `cid:`-referenceable image
 * part with its bytes. Typed for real rather than aliased to `any`
 * because the renderer matches on every field.
 */
export interface InlineImagePart {
  partId: number
  /** RFC 2392 Content-ID without angle brackets, when the part had one. */
  contentId: string | null
  filename: string
  mime: string
  base64: string
}
export type InviteSummary = any
export type LinkVerdict = any
export type LoginFlowInit = any
export type MailingListView = any
export type NextcloudAccount = any
export type NextcloudGroupView = any
export type NextcloudMapsCapability = any
export type NextcloudShareResult = any
export type NextcloudShareRow = any
export type NextcloudUserLookup = any

/**
 * Backend `SystemAccessView` — how far the user has let the app into
 * each of the device's own databases.
 *
 * Grades are strings rather than a union so a future iOS release
 * inventing a fourth one can't make the settings screen fail to
 * parse; the UI treats anything it doesn't recognise as "not
 * granted".
 */
export interface SystemAccessView {
  calendars: string
  reminders: string
  contacts: string
  /** False where the platform has no bridge — the settings screen
   *  hides the whole section rather than offering a dead button. */
  available: boolean
}
export type Note = any
/** Backend `OfficeOpenResult` — where an attachment was parked on
 *  the user's Nextcloud and the Files deep link that renders it. */
export interface OfficeOpenResult {
  url: string
  tempPath: string
}
export type OutboxRowDto = any
export type OutboxSourceRef = any
export type OutgoingEmail = any
export type ParticipantSource = any
export type PgpKeyStatus = any
export type PgpPublicKeyDto = any
export type ProbedCert = any
export type ProviderPreset = any
export type RepliedToRef = any
export type SavedDraft = any
export type SearchFilters = any
export type SearchHit = any
export type SearchScope = any
export type SentReceiptStatus = any
export type SettingsSyncStateView = any
export type SmimeCertDto = any
export type SmimeCertStatus = any
export type SyncCalendarsReport = any
export type SyncContactsReport = any
export type SyncStatus = any
export type TalkRoom = any
export type Task = any
export type TaskList = any
/**
 * Backend `unkai_core::models::TrustedCert` — one TLS leaf cert the
 * user explicitly trusted for an account. Typed alongside `Account`
 * (#534) since the account row embeds the list.
 */
export interface TrustedCert {
  /** Raw DER bytes as a JSON byte array (Rust `Vec<u8>`). */
  der: number[]
  /** SHA-256 fingerprint, lowercase hex with `:` separators. */
  sha256: string
  host: string
  /** Unix epoch seconds when the cert was trusted. */
  added_at: number
}
export type UrlhausStatus = any
export type WipePolicyView = any
