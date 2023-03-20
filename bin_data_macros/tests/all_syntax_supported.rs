#![allow(dead_code)]

use bin_data::data::Decode;
use bin_data::named_args::Endian;
use bin_data_macros::bin_data;

bin_data! {
    /// some docs
    #[derive(Debug, Copy, Clone, PartialEq)]
    #[bin_data(endian = Endian::Little)]
    pub struct Test {
        #[bin_data(endian = Endian::Big)]
        some_private_field: i64,
        pub some_pub_field: u8,
        pub(in self) some_fancy_visibility: u32,
        @pad(3),
        #[bin_data(encode = 42.0)]
        let temporary: f32,
        #[bin_data(decode = temporary)]
        pub move_data: f32,
        @magic([0x12, 0x34, 0x56, 0x78]),
    }
}

#[test]
fn test_decode() {
    let input = [
        0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, // local big endian
        42,
        0xEF, 0xBE, 0xAD, 0xDE, // "dead beef"
        10, 20, 30, // padding: dropped
        0xDB, 0x0F, 0x49, 0x40, // PI
        0x12, 0x34, 0x56, 0x78, // magic
    ];
    assert_eq!(std::f32::consts::PI.to_le_bytes(), [0xDB, 0x0F, 0x49, 0x40]);
    let test = Test::decode(&mut input.as_ref()).unwrap();
    let expected = Test {
        some_private_field: 0x11_22_33_44_55_66_77_88,
        some_pub_field: 42,
        some_fancy_visibility: 0xDEAD_BEEF,
        move_data: std::f32::consts::PI,
    };
    assert_eq!(test, expected);
}

fn main() {}
