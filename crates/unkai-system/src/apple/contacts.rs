//! The Contacts framework, exchanged as vCard text.
//!
//! `CNContactVCardSerialization` is the reason this module is short:
//! Apple will hand us a whole contact as a vCard and take one back,
//! so the system address book reaches the app through the very same
//! parser (`unkai-carddav`) every CardDAV server does. Labelled
//! phone numbers, postal addresses and photos are then somebody
//! else's already-tested problem.
//!
//! The one place we still touch individual properties is **update**:
//! `CNSaveRequest` can only write a contact that was fetched from the
//! store, and a vCard parsed into a fresh `CNContact` is not that. So
//! an update fetches the real record and copies the parsed values
//! onto it, field by field.
//!
//! `note` is deliberately never read or written: since iOS 13 the
//! notes key needs a per-app entitlement from Apple, and asking for a
//! key you don't hold makes the *whole* fetch throw.

use std::cell::RefCell;
use std::ptr::NonNull;

use objc2::AnyThread;
use objc2::rc::Retained;
use objc2::runtime::{Bool, ProtocolObject};
use objc2_contacts::{
    CNAuthorizationStatus, CNContact, CNContactFetchRequest, CNContactImageDataKey, CNContactStore,
    CNContactVCardSerialization, CNEntityType, CNKeyDescriptor, CNMutableContact, CNSaveRequest,
};
use objc2_foundation::{NSArray, NSCopying, NSData, NSMutableCopying};

use super::{Waiter, backend_error, nsstring};
use crate::{PermissionState, SystemContact, SystemEntity, SystemError};

impl From<CNAuthorizationStatus> for PermissionState {
    fn from(status: CNAuthorizationStatus) -> Self {
        match status {
            CNAuthorizationStatus::NotDetermined => PermissionState::NotDetermined,
            CNAuthorizationStatus::Authorized => PermissionState::Granted,
            // iOS 18's "limited" grade exposes only contacts the user
            // hand-picked. That is a *usable* address book — smaller
            // than the real one, but everything in it is readable and
            // writable — so it counts as granted rather than denied.
            CNAuthorizationStatus::Limited => PermissionState::Granted,
            _ => PermissionState::Denied,
        }
    }
}

pub(super) fn access_status() -> PermissionState {
    unsafe { CNContactStore::authorizationStatusForEntityType(CNEntityType::Contacts) }.into()
}

pub(super) fn request_access() -> Result<(), SystemError> {
    let store = unsafe { CNContactStore::new() };
    let waiter = Waiter::new();
    let block = waiter.access_block();
    // SAFETY: `block` outlives the call — `waiter.wait` below does not
    // return until the completion has fired or timed out.
    unsafe { store.requestAccessForEntityType_completionHandler(CNEntityType::Contacts, &block) };
    waiter.wait(SystemEntity::Contacts)
}

/// The key set `CNContactVCardSerialization` needs to be able to
/// write a complete vCard, **plus the contact image**. Asking for
/// exactly this — and nothing more — is what keeps the fetch clear
/// of the entitlement-gated notes key.
///
/// `imageData` is not part of `descriptorForRequiredKeys`, and it
/// has to be fetched explicitly: reading `imageData` off a contact
/// whose fetch didn't ask for it throws rather than returning `nil`.
fn vcard_keys() -> Retained<NSArray<ProtocolObject<dyn CNKeyDescriptor>>> {
    let descriptor = unsafe { CNContactVCardSerialization::descriptorForRequiredKeys() };
    // SAFETY: the key constant is a static `NSString`, and `NSString`
    // conforms to `CNKeyDescriptor`.
    let image = ProtocolObject::from_retained(unsafe { CNContactImageDataKey }.copy());
    NSArray::from_retained_slice(&[descriptor, image])
}

pub fn list_contacts() -> Result<Vec<SystemContact>, SystemError> {
    super::require_access(SystemEntity::Contacts)?;
    let store = unsafe { CNContactStore::new() };
    let request = unsafe {
        CNContactFetchRequest::initWithKeysToFetch(CNContactFetchRequest::alloc(), &vcard_keys())
    };

    // The enumeration is synchronous on this thread, so the block can
    // borrow — no `Arc<Mutex<…>>` needed, and each contact is turned
    // into plain Rust before the next one is handed over.
    let collected: RefCell<Vec<SystemContact>> = RefCell::new(Vec::new());
    let block = block2::StackBlock::new(|contact: NonNull<CNContact>, _stop: NonNull<Bool>| {
        // SAFETY: the framework hands the block a live contact for the
        // duration of the callback.
        let contact = unsafe { contact.as_ref() };
        match to_system_contact(contact) {
            Ok(row) => collected.borrow_mut().push(row),
            // One unserialisable contact must not cost the user the
            // other 400. Log it and carry on — the same policy the
            // CardDAV sync applies to an unparseable vCard.
            Err(e) => tracing::warn!("skipping a system contact that would not serialise: {e}"),
        }
    });

    let mut error = None;
    // SAFETY: the request and the block both outlive the call.
    let ok = unsafe {
        store.enumerateContactsWithFetchRequest_error_usingBlock(&request, Some(&mut error), &block)
    };
    if !ok {
        return Err(error
            .map(backend_error)
            .unwrap_or_else(|| SystemError::Backend("the contact enumeration failed".into())));
    }

    Ok(collected.into_inner())
}

fn to_system_contact(contact: &CNContact) -> Result<SystemContact, SystemError> {
    let id = unsafe { contact.identifier() }.to_string();
    let array = NSArray::from_slice(&[contact]);
    let data = unsafe { CNContactVCardSerialization::dataWithContacts_error(&array) }
        .map_err(backend_error)?;
    let vcard = String::from_utf8(data.to_vec())
        .map_err(|_| SystemError::Backend("the system vCard was not valid UTF-8".into()))?;
    let photo = unsafe { contact.imageData() }.map(|d| d.to_vec());
    Ok(SystemContact {
        vcard: with_photo(&with_uid(&vcard, &id), photo.as_deref()),
        id,
    })
}

/// Force the vCard's `UID` to the Contacts identifier.
///
/// Apple's serialiser doesn't emit a `UID` at all, and the app keys
/// contacts by the one in the vCard — so without this every sync
/// would mint fresh identities and the cache would grow a duplicate
/// of the address book on each run.
fn with_uid(vcard: &str, id: &str) -> String {
    let mut out = String::with_capacity(vcard.len() + id.len() + 8);
    let mut uid_written = false;
    for line in vcard.lines() {
        let upper = line.to_ascii_uppercase();
        // Drop any UID the source did carry, so we never emit two.
        if upper.starts_with("UID:") || upper.starts_with("UID;") {
            continue;
        }
        out.push_str(line);
        out.push_str("\r\n");
        // Straight after BEGIN:VCARD is the one position that's valid
        // in both vCard 3.0 and 4.0.
        if !uid_written && upper.starts_with("BEGIN:VCARD") {
            out.push_str("UID:");
            out.push_str(id);
            out.push_str("\r\n");
            uid_written = true;
        }
    }
    out
}

/// Splice the contact's image into the vCard as a `PHOTO` property.
///
/// `CNContactVCardSerialization` silently omits the image — it emits
/// every other property it was given keys for, but never the photo.
/// So the bytes are fetched separately (see [`vcard_keys`]) and
/// written in here, in the vCard 3.0 inline form Apple's own
/// serialiser targets.
///
/// The encoding is the one `unkai-carddav`'s parser already reads
/// (`PHOTO;ENCODING=b;TYPE=JPEG:<base64>`), so a device contact's
/// avatar lands in `photo_data` by exactly the same path a
/// Nextcloud one does — no branch anywhere downstream.
///
/// `TYPE=JPEG` is asserted rather than sniffed: Contacts normalises
/// what it stores to JPEG, the vCard `TYPE` parameter is a hint the
/// app's own decoder ignores in favour of the bytes, and nothing
/// downstream renders from it.
fn with_photo(vcard: &str, photo: Option<&[u8]>) -> String {
    let Some(bytes) = photo.filter(|b| !b.is_empty()) else {
        return vcard.to_string();
    };
    // A photo the serialiser did emit after all (a future OS
    // fixing this) wins — don't write a second one.
    if vcard
        .lines()
        .any(|l| l.to_ascii_uppercase().starts_with("PHOTO"))
    {
        return vcard.to_string();
    }

    let line = format!("PHOTO;ENCODING=b;TYPE=JPEG:{}", base64(bytes));
    let mut out = String::with_capacity(vcard.len() + line.len() + 8);
    let mut written = false;
    for l in vcard.lines() {
        // Immediately before END:VCARD, so the property can't land
        // outside the card if the source has trailing blank lines.
        if !written && l.to_ascii_uppercase().starts_with("END:VCARD") {
            fold(&mut out, &line);
            written = true;
        }
        out.push_str(l);
        out.push_str("\r\n");
    }
    out
}

/// Append a property, folded to 75 octets per line as RFC 6350 §3.2
/// requires. A base64 photo is thousands of characters on one
/// logical line, and servers and parsers alike have been known to
/// truncate an unfolded one.
fn fold(out: &mut String, line: &str) {
    // ASCII throughout (property name, parameters and base64), so
    // byte offsets and character offsets agree and a split can't
    // land mid-character.
    let bytes = line.as_bytes();
    let mut start = 0;
    while start < bytes.len() {
        let end = (start + 75).min(bytes.len());
        if start > 0 {
            out.push(' ');
        }
        out.push_str(&line[start..end]);
        out.push_str("\r\n");
        start = end;
    }
}

/// Standard base64, no line breaks — [`fold`] owns the wrapping.
///
/// Hand-rolled rather than pulled in as a dependency: it is fifteen
/// lines, and this crate's whole point is to stay a thin bridge.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/* ── Writes ──────────────────────────────────────────────────────*/

pub fn create_contact(vcard: &str) -> Result<String, SystemError> {
    super::require_access(SystemEntity::Contacts)?;
    let store = unsafe { CNContactStore::new() };
    let parsed = parse_vcard(vcard)?;
    let contact = parsed.mutableCopy();

    let request = unsafe { CNSaveRequest::new() };
    // A `nil` container means the user's default account — the same
    // place the Contacts app puts a new card.
    unsafe { request.addContact_toContainerWithIdentifier(&contact, None) };
    unsafe { store.executeSaveRequest_error(&request) }.map_err(backend_error)?;

    Ok(unsafe { contact.identifier() }.to_string())
}

pub fn update_contact(id: &str, vcard: &str) -> Result<(), SystemError> {
    super::require_access(SystemEntity::Contacts)?;
    let store = unsafe { CNContactStore::new() };
    let existing = fetch_mutable(&store, id)?;
    let parsed = parse_vcard(vcard)?;
    copy_fields(&parsed, &existing);

    let request = unsafe { CNSaveRequest::new() };
    unsafe { request.updateContact(&existing) };
    unsafe { store.executeSaveRequest_error(&request) }.map_err(backend_error)
}

pub fn delete_contact(id: &str) -> Result<(), SystemError> {
    super::require_access(SystemEntity::Contacts)?;
    let store = unsafe { CNContactStore::new() };
    let existing = fetch_mutable(&store, id)?;

    let request = unsafe { CNSaveRequest::new() };
    unsafe { request.deleteContact(&existing) };
    unsafe { store.executeSaveRequest_error(&request) }.map_err(backend_error)
}

fn fetch_mutable(
    store: &CNContactStore,
    id: &str,
) -> Result<Retained<CNMutableContact>, SystemError> {
    let contact = unsafe {
        store.unifiedContactWithIdentifier_keysToFetch_error(&nsstring(id), &vcard_keys())
    }
    .map_err(|_| SystemError::NotFound(format!("contact '{id}'")))?;
    Ok(contact.mutableCopy())
}

fn parse_vcard(vcard: &str) -> Result<Retained<CNContact>, SystemError> {
    let data = NSData::with_bytes(vcard.as_bytes());
    let contacts = unsafe { CNContactVCardSerialization::contactsWithData_error(&data) }
        .map_err(backend_error)?;
    contacts
        .to_vec()
        .into_iter()
        .next()
        .ok_or_else(|| SystemError::Backend("the vCard held no contact".into()))
}

/// Copy every property we round-trip from a parsed vCard onto the
/// record the store actually knows about.
///
/// This is the price of `CNSaveRequest` only accepting store-fetched
/// objects. The list is deliberately the same set
/// `descriptorForRequiredKeys` covers, minus `note` (entitlement-
/// gated) — anything outside it is left as the device has it rather
/// than silently cleared.
fn copy_fields(from: &CNContact, to: &CNMutableContact) {
    unsafe {
        to.setNamePrefix(&from.namePrefix());
        to.setGivenName(&from.givenName());
        to.setMiddleName(&from.middleName());
        to.setFamilyName(&from.familyName());
        to.setPreviousFamilyName(&from.previousFamilyName());
        to.setNameSuffix(&from.nameSuffix());
        to.setNickname(&from.nickname());
        to.setOrganizationName(&from.organizationName());
        to.setDepartmentName(&from.departmentName());
        to.setJobTitle(&from.jobTitle());
        to.setPhoneticGivenName(&from.phoneticGivenName());
        to.setPhoneticMiddleName(&from.phoneticMiddleName());
        to.setPhoneticFamilyName(&from.phoneticFamilyName());
        to.setPhoneNumbers(&from.phoneNumbers());
        to.setEmailAddresses(&from.emailAddresses());
        to.setPostalAddresses(&from.postalAddresses());
        to.setUrlAddresses(&from.urlAddresses());
        to.setBirthday(from.birthday().as_deref());
        to.setDates(&from.dates());
        // A vCard without a PHOTO must not wipe the photo the phone
        // has: `imageData` is `None` both for "no photo" and for "not
        // fetched", and we can't tell those apart here.
        if let Some(image) = from.imageData() {
            to.setImageData(Some(&image));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{base64, with_photo, with_uid};

    #[test]
    fn uid_is_inserted_after_begin() {
        let card = "BEGIN:VCARD\nVERSION:3.0\nFN:Alex Morgan\nEND:VCARD\n";
        let out = with_uid(card, "ABC-123");
        assert!(out.starts_with("BEGIN:VCARD\r\nUID:ABC-123\r\n"));
        assert_eq!(out.matches("UID:").count(), 1);
    }

    #[test]
    fn base64_matches_the_reference_vectors() {
        // RFC 4648 §10 — including both padding shapes, which is
        // where a hand-rolled encoder goes wrong.
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn a_photo_is_spliced_in_before_end_vcard() {
        let card = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Alex Morgan\r\nEND:VCARD\r\n";
        let out = with_photo(card, Some(b"foobar"));
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[3], "PHOTO;ENCODING=b;TYPE=JPEG:Zm9vYmFy");
        assert_eq!(lines[4], "END:VCARD");
    }

    #[test]
    fn no_photo_leaves_the_card_alone() {
        let card = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Alex Morgan\r\nEND:VCARD\r\n";
        assert_eq!(with_photo(card, None), card);
        assert_eq!(with_photo(card, Some(&[])), card);
    }

    #[test]
    fn a_photo_the_serialiser_emitted_is_not_duplicated() {
        let card = "BEGIN:VCARD\r\nPHOTO;ENCODING=b;TYPE=JPEG:AAAA\r\nEND:VCARD\r\n";
        let out = with_photo(card, Some(b"foobar"));
        assert_eq!(out.matches("PHOTO").count(), 1);
    }

    #[test]
    fn a_long_photo_line_is_folded() {
        // 3 KB of image is ~4 KB of base64 — one logical line that
        // has to come out as many physical ones, each continuation
        // marked by a leading space.
        let out = with_photo("BEGIN:VCARD\r\nEND:VCARD\r\n", Some(&vec![0x42u8; 3000]));
        let photo: Vec<&str> = out
            .lines()
            .skip_while(|l| !l.starts_with("PHOTO"))
            .take_while(|l| !l.starts_with("END:"))
            .collect();
        assert!(
            photo.len() > 50,
            "expected the photo to fold over many lines"
        );
        assert!(photo.iter().all(|l| l.len() <= 76));
        assert!(photo[1..].iter().all(|l| l.starts_with(' ')));
        // Unfolding has to give the original property back.
        let rejoined: String = photo
            .iter()
            .enumerate()
            .map(|(i, l)| if i == 0 { *l } else { &l[1..] })
            .collect();
        assert_eq!(
            rejoined,
            format!("PHOTO;ENCODING=b;TYPE=JPEG:{}", base64(&[0x42u8; 3000]))
        );
    }

    #[test]
    fn an_existing_uid_is_replaced_not_duplicated() {
        let card = "BEGIN:VCARD\nVERSION:3.0\nUID:stale\nFN:Alex Morgan\nEND:VCARD\n";
        let out = with_uid(card, "ABC-123");
        assert!(out.contains("UID:ABC-123"));
        assert!(!out.contains("stale"));
        assert_eq!(out.matches("UID:").count(), 1);
    }
}
