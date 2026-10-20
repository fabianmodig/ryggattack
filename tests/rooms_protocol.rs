use ryggattack::network::{VERSION, decode_client};
#[test]
fn room_join_is_versioned_and_bounded() {
    assert!(
        decode_client(&format!(
            r#"{{"Hello":{{"version":{VERSION},"name":"Ada","room":"0123456789abcdef0123"}}}}"#
        ))
        .is_ok()
    );
    for code in ["", "ABC", "0123456789abcdef01234", "../../secret"] {
        assert!(
            decode_client(&format!(
                r#"{{"Hello":{{"version":{VERSION},"name":"Ada","room":"{code}"}}}}"#
            ))
            .is_err()
        );
    }
    assert!(decode_client(r#"{"Hello":{"version":1,"name":"Ada"}}"#).is_err());
}
