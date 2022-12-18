use crate::model::{Hex16, Raw16};

#[test]
fn test_hex16() {
    let id = Hex16::from(0x61d78641a649a);
    assert_eq!(id.to_string(), "61d78641a649a");
    let id = Hex16::from_hex(b"61d78641a649a").unwrap();
    assert_eq!(id.to_string(), "61d78641a649a");
    let id = Hex16::from_hex("61d78641a649a").unwrap();
    assert_eq!(id.to_string(), "61d78641a649a");
}

#[test]
fn test_raw16() {
    let id = Raw16::from_string(b"foobarbaz");
    assert_eq!(id.to_string(), "foobarbaz");
    let id = Raw16::from_string("foobarbaz");
    assert_eq!(id.to_string(), "foobarbaz");
}
