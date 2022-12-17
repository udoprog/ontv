use crate::model::Id;

#[test]
fn test_id() {
    let id = Id::from(0x61d78641a649a);
    assert_eq!(id.to_string(), "61d78641a649a");
    let id = Id::from_hex(b"61d78641a649a").unwrap();
    assert_eq!(id.to_string(), "61d78641a649a");
    let id = Id::from_hex("61d78641a649a").unwrap();
    assert_eq!(id.to_string(), "61d78641a649a");
}
