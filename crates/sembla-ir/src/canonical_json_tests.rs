use super::to_canonical_string;
use serde::Serialize;

#[derive(Serialize)]
struct UnsortedFields {
    z_key: u8,
    a_key: u8,
}

#[test]
fn serde_json_objects_are_btree_sorted() {
    let value = UnsortedFields { z_key: 1, a_key: 2 };
    assert_eq!(
        to_canonical_string(&value).unwrap(),
        r#"{"a_key":2,"z_key":1}"#
    );
}
