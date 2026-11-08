//! Invitation data, independent of clipboard, menus, transport and future platforms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invitation {
    pub address: String,
    pub code: String,
}

impl Invitation {
    pub fn browser_link(&self, web_client: &str) -> Result<String, &'static str> {
        if !valid_endpoint(&self.address) || !crate::network::valid_code(&self.code) {
            return Err("Invalid room invitation.");
        }
        let mut url = url::Url::parse(web_client).map_err(|_| "Invalid browser client URL.")?;
        if !(web_client.starts_with("http://") || web_client.starts_with("https://"))
            || web_client.len() > 2048
            || web_client
                .chars()
                .any(|ch| ch.is_whitespace() || ch.is_control())
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err("Invalid browser client URL.");
        }
        if url.scheme() == "https" && self.address.starts_with("ws://") {
            return Err(
                "For a ws:// backend, set RYGGATTACK_WEB_URL to your HTTP-hosted browser client, or copy the room code.",
            );
        }
        url.set_query(None);
        url.set_fragment(None);
        url.query_pairs_mut()
            .append_pair("online", &self.address)
            .append_pair("room", &self.code);
        Ok(url.to_string())
    }

    pub fn parse(text: &str, current_address: &str) -> Result<Self, &'static str> {
        const INVALID: &str = "Enter a valid room code or http(s) invitation link.";
        if text.len() > 4096 {
            return Err(INVALID);
        }
        let text = text.trim();
        let (address, code) = if crate::network::valid_code(text) {
            (current_address.to_owned(), text.to_owned())
        } else {
            if !(text.starts_with("https://") || text.starts_with("http://"))
                || text.chars().any(char::is_whitespace)
            {
                return Err(INVALID);
            }
            let url = url::Url::parse(text).map_err(|_| INVALID)?;
            if url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.fragment().is_some()
            {
                return Err(INVALID);
            }
            let (mut address, mut code) = (None, None);
            for (key, value) in url.query_pairs() {
                match key.as_ref() {
                    "online" if address.is_none() => address = Some(value.into_owned()),
                    "room" if code.is_none() => code = Some(value.into_owned()),
                    "online" | "room" => return Err(INVALID),
                    _ => {}
                }
            }
            (address.ok_or(INVALID)?, code.ok_or(INVALID)?)
        };
        if !valid_endpoint(&address) || !crate::network::valid_code(&code) {
            return Err(INVALID);
        }
        Ok(Self { address, code })
    }
}

pub(crate) fn valid_endpoint(address: &str) -> bool {
    address.len() <= 256
        && (address.starts_with("ws://") || address.starts_with("wss://"))
        && !address
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_control())
        && url::Url::parse(address).is_ok_and(|url| {
            url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && url.fragment().is_none()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invitation_link_roundtrips_and_contains_only_public_join_fields() {
        let invite = Invitation {
            address: "wss://example.test:9443/custom/ws?route=a%2Bb".into(),
            code: "0123456789abcdef0123".into(),
        };
        let link = invite
            .browser_link(
                "https://ryggattack.modig.online/?name=Old&credential=do-not-share#private",
            )
            .unwrap();
        let url = url::Url::parse(&link).unwrap();
        assert_eq!(
            url.query_pairs()
                .map(|(k, _)| k.into_owned())
                .collect::<Vec<_>>(),
            ["online", "room"]
        );
        assert!(url.fragment().is_none());
        assert!(!link.contains("do-not-share"));
        assert_eq!(
            Invitation::parse(&link, "ws://localhost:8787").unwrap(),
            invite
        );
    }

    #[test]
    fn code_only_preserves_selected_backend_verbatim() {
        let invite =
            Invitation::parse(" 0123456789abcdef0123\n", "wss://Example.test/ws?route=%2f")
                .unwrap();
        assert_eq!(invite.address, "wss://Example.test/ws?route=%2f");
    }

    #[test]
    fn malformed_or_ambiguous_invitations_are_rejected() {
        let base = "https://ryggattack.modig.online/?online=wss%3A%2F%2Fexample.test%2Fws&room=0123456789abcdef0123";
        for text in [
            "",
            "0123456789ABCDEF0123",
            "0123456789abcdef01234",
            "https://example.test/?room=0123456789abcdef0123",
            "https://user:password@example.test/?room=0123456789abcdef0123",
            "file:///tmp/invitation",
            "javascript:alert(1)",
        ] {
            assert!(
                Invitation::parse(text, "wss://example.test/ws").is_err(),
                "accepted invalid input"
            );
        }
        for tail in [
            "&room=0123456789abcdef0123",
            "&online=wss%3A%2F%2Fother.test%2Fws",
            "#credential",
        ] {
            assert!(Invitation::parse(&format!("{base}{tail}"), "wss://example.test/ws").is_err());
        }
        for backend in [
            "https://example.test/ws",
            "wss:example.test",
            "wss://user:password@example.test/ws",
            "wss://example.test/ws#credential",
            "wss://example.test/\0",
        ] {
            assert!(
                Invitation::parse("0123456789abcdef0123", backend).is_err(),
                "accepted invalid backend"
            );
        }
        assert!(Invitation::parse(&" ".repeat(5000), "wss://example.test/ws").is_err());
    }

    #[test]
    fn browser_invitation_prefills_endpoint_and_code_without_admission() {
        let invite = Invitation::parse("https://ryggattack.modig.online/?online=wss%3A%2F%2Fryggattack.modig.online%2Fws&room=0123456789abcdef0123", "ws://127.0.0.1:8787").unwrap();
        assert_eq!(invite.address, "wss://ryggattack.modig.online/ws");
        assert_eq!(invite.code, "0123456789abcdef0123");
    }
}
