//! Smoke test against the development fixture server.
//!
//! `devtools/fake_mail_server.py` speaks the slice of IMAP this
//! client uses. That claim is only worth anything if something
//! checks it, and checking it here — against the real client, not a
//! hand-written expectation — is what catches "the fixture drifted
//! from what we actually send".
//!
//! Ignored by default because it needs the server running:
//!
//!     python3 devtools/fake_mail_server.py &
//!     cargo test -p unkai-imap --test fixture_server -- --ignored --nocapture

use unkai_core::models::TrustedCert;

const HOST: &str = "localhost";
const IMAP_PORT: u16 = 9993;

/// Trust whatever certificate the fixture generated, the same way
/// the app does after the user taps "Trust and continue".
async fn trusted() -> Vec<TrustedCert> {
    let chain = unkai_imap::probe_server_certificate(HOST, IMAP_PORT)
        .await
        .expect("fixture server reachable (is it running?)");
    chain
        .into_iter()
        .map(|der| {
            let sha256 = unkai_core::tls::fingerprint_sha256(&der);
            TrustedCert {
                der,
                sha256,
                host: HOST.to_string(),
                added_at: 0,
            }
        })
        .collect()
}

#[tokio::test]
#[ignore = "needs devtools/fake_mail_server.py running"]
async fn lists_folders_and_envelopes() {
    let certs = trusted().await;
    let mut client =
        unkai_imap::ImapClient::connect(HOST, IMAP_PORT, "you@example.com", "x", &certs)
            .await
            .expect("login");

    let folders = client.list_folders().await.expect("LIST");
    assert!(
        folders.iter().any(|f| f.name.eq_ignore_ascii_case("INBOX")),
        "fixture should serve an INBOX, got {folders:?}"
    );

    let batch = client
        .fetch_envelopes("INBOX", 50, None)
        .await
        .expect("FETCH envelopes");
    assert!(
        !batch.envelopes.is_empty(),
        "fixture INBOX should not come back empty"
    );

    let newest = &batch.envelopes[0];
    assert!(!newest.from.is_empty(), "envelope carries a From");
    assert!(!newest.subject.is_empty(), "envelope carries a Subject");

    let message = client
        .fetch_message("INBOX", newest.uid, "fixture-account")
        .await
        .expect("FETCH body");
    assert!(
        message.body_html.is_some() || message.body_text.is_some(),
        "message has a body"
    );

    let _ = client.logout().await;
}

/// The reconcile search the sync path runs on every poll.
///
/// `poll_folder` no longer asks for every UID in the mailbox — it
/// bounds the search to the range the cache actually holds. That is a
/// different IMAP command (`UID SEARCH UID low:high` rather than `UID
/// SEARCH ALL`), so it needs its own check that a real server answers
/// it the way the reconcile assumes: every live UID inside the range,
/// and nothing outside it.
#[tokio::test]
#[ignore = "needs devtools/fake_mail_server.py running"]
async fn range_scoped_uid_search_matches_the_full_one() {
    let certs = trusted().await;
    let mut client =
        unkai_imap::ImapClient::connect(HOST, IMAP_PORT, "you@example.com", "x", &certs)
            .await
            .expect("login");

    let mut all = client.list_all_uids("INBOX").await.expect("UID SEARCH ALL");
    all.sort_unstable();
    assert!(
        all.len() >= 2,
        "fixture INBOX needs a few messages, got {all:?}"
    );

    // The whole range must reproduce the full listing exactly.
    let low = *all.first().unwrap();
    let high = *all.last().unwrap();
    let mut full_range = client
        .list_uids_in_range("INBOX", low, high)
        .await
        .expect("UID SEARCH over the full range");
    full_range.sort_unstable();
    assert_eq!(all, full_range, "the bounded search must not lose UIDs");

    // A narrower window returns exactly the UIDs inside it — this is
    // what keeps the reconcile from declaring a message a ghost just
    // because it sits outside the cached window.
    let mut narrow = client
        .list_uids_in_range("INBOX", low, low)
        .await
        .expect("UID SEARCH over one UID");
    narrow.sort_unstable();
    assert_eq!(narrow, vec![low]);

    let _ = client.logout().await;
}

/// The SMTP half of the fixture, driven through the real client.
///
/// Lives in this crate rather than `unkai-smtp`'s tests because the
/// certificate has to be probed first, and that probe is an IMAP-crate
/// function — the same order the app follows.
#[tokio::test]
#[ignore = "needs devtools/fake_mail_server.py running"]
async fn sends_through_smtp() {
    let certs = trusted().await;
    let client = unkai_smtp::SmtpClient::connect("localhost", 9465, "you@example.com", "x", &certs)
        .await
        .expect("SMTP connect");

    let email = unkai_core::models::OutgoingEmail {
        from: "you@example.com".into(),
        to: vec!["you@example.com".into()],
        cc: vec![],
        bcc: vec![],
        reply_to: None,
        subject: "Fixture round-trip".into(),
        body_text: Some("Sent by the fixture smoke test.".into()),
        body_html: None,
        attachments: vec![],
        calendar_part: None,
        skip_sent_copy: true,
        in_reply_to: None,
        references: vec![],
        encryption_mode: None,
        signing_enabled: false,
        request_read_receipt: false,
        request_delivery_receipt: false,
    };

    client.send(&email).await.expect("SMTP send");
}
