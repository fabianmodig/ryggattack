//! Clipboard is only a delivery adapter; invitations contain public join data.
use super::{Action, Entry};
use crate::invitation::Invitation;
use bevy::prelude::Clipboard;

pub(super) fn web_client(address: &str) -> String {
    std::env::var("RYGGATTACK_WEB_URL").unwrap_or_else(|_| {
        if url::Url::parse(address)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned))
            .as_deref()
            == Some("ryggattack-beta.modig.online")
        {
            "https://ryggattack-beta.modig.online/".into()
        } else {
            "https://ryggattack.modig.online/".into()
        }
    })
}

pub(super) fn copy(
    invite: &Invitation,
    code_only: bool,
    web_client: &str,
    write: impl FnOnce(&str) -> Result<(), String>,
) -> Result<&'static str, String> {
    let text = if code_only {
        if !crate::network::valid_code(&invite.code) {
            return Err("Invalid room code.".into());
        }
        invite.code.clone()
    } else {
        invite.browser_link(web_client).map_err(str::to_owned)?
    };
    write(&text).map_err(|error| format!("Clipboard write failed: {error}"))?;
    Ok(if code_only {
        "Copied room code!"
    } else {
        "Copied invite link!"
    })
}

pub(super) fn read(clipboard: Option<&mut Clipboard>) -> Result<String, String> {
    clipboard
        .ok_or("System clipboard is unavailable.")?
        .fetch_text()
        .poll_result()
        .ok_or("Clipboard read did not complete. Try again.")?
        .map_err(|_| "Could not read text from the system clipboard.".into())
}

pub(super) fn paste(
    entry: &mut Entry,
    field: Option<Action>,
    replace: bool,
    text: &str,
) -> Result<&'static str, String> {
    if text.len() > 4096 {
        return Err("Clipboard text is too long.".into());
    }
    if matches!(field, Some(Action::Name | Action::Address)) {
        let (value, max) = if field == Some(Action::Name) {
            (&mut entry.name, 24)
        } else {
            (&mut entry.address, 256)
        };
        if replace {
            value.clear();
        }
        for ch in text.chars().filter(|ch| !ch.is_control()) {
            if value.chars().count() < max {
                value.push(ch);
            }
        }
        return Ok("Text pasted.");
    }
    let invite = Invitation::parse(text, &entry.address).map_err(str::to_owned)?;
    // Validate everything before changing either field. No transport is opened.
    entry.address = invite.address;
    entry.code = invite.code;
    Ok("Invitation pasted. Select JOIN ROOM to connect.")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn invite() -> Invitation {
        Invitation {
            address: "wss://ryggattack.modig.online/ws".into(),
            code: "0123456789abcdef0123".into(),
        }
    }
    fn entry() -> Entry {
        Entry {
            address: "ws://localhost:8787".into(),
            name: "Player".into(),
            code: String::new(),
        }
    }
    #[test]
    fn copy_writes_a_browser_compatible_invitation_before_confirming_success() {
        let invite = invite();
        let mut copied = String::new();
        let notice = copy(&invite, false, "https://ryggattack.modig.online/", |text| {
            copied = text.into();
            Ok(())
        })
        .unwrap();
        assert_eq!(
            Invitation::parse(&copied, "ws://localhost:8787").unwrap(),
            invite
        );
        assert!(notice.starts_with("Copied"));
        assert_eq!(
            copy(&invite, true, "unused", |text| {
                assert_eq!(text, invite.code);
                Ok(())
            })
            .unwrap(),
            "Copied room code!"
        );
    }
    #[test]
    fn clipboard_write_failure_never_reports_copied() {
        let error = copy(&invite(), false, "https://ryggattack.modig.online/", |_| {
            Err("unavailable".into())
        })
        .unwrap_err();
        assert!(error.contains("Clipboard"));
        assert!(!error.contains("Copied"));
        assert!(read(None).unwrap_err().contains("clipboard"));
    }
    #[test]
    fn insecure_backend_requires_an_http_browser_client_before_writing() {
        let invite = Invitation {
            address: "ws://localhost:8787".into(),
            ..invite()
        };
        assert!(
            copy(
                &invite,
                false,
                "https://ryggattack.modig.online/",
                |_| panic!("must not write a broken browser link")
            )
            .is_err()
        );
        let mut copied = String::new();
        copy(&invite, false, "http://localhost:8080/", |text| {
            copied = text.into();
            Ok(())
        })
        .unwrap();
        assert_eq!(
            Invitation::parse(&copied, "wss://other.test/ws").unwrap(),
            invite
        );
    }
    #[test]
    fn invitation_paste_prefills_both_fields_without_changing_the_name() {
        let mut entry = entry();
        let link = invite()
            .browser_link("https://ryggattack.modig.online/")
            .unwrap();
        assert!(
            paste(&mut entry, Some(Action::Code), false, &link)
                .unwrap()
                .contains("JOIN ROOM")
        );
        assert_eq!(entry.address, invite().address);
        assert_eq!(entry.code, invite().code);
        assert_eq!(entry.name, "Player");
    }
    #[test]
    fn invalid_paste_keeps_both_existing_connection_fields() {
        let mut entry = entry();
        entry.code = invite().code;
        assert!(
            paste(
                &mut entry,
                None,
                true,
                "https://bad.test/?online=wss://other.test&room=invalid"
            )
            .is_err()
        );
        assert_eq!(entry.address, "ws://localhost:8787");
        assert_eq!(entry.code, "0123456789abcdef0123");
    }
    #[test]
    fn ordinary_field_paste_respects_unicode_length_and_replace_selection() {
        let mut entry = entry();
        paste(&mut entry, Some(Action::Name), true, "Åda\n").unwrap();
        assert_eq!(entry.name, "Åda");
        paste(&mut entry, Some(Action::Name), false, &"x".repeat(30)).unwrap();
        assert_eq!(entry.name.chars().count(), 24);
        assert_eq!(entry.address, "ws://localhost:8787");
    }
}
