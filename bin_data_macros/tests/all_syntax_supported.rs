#![allow(dead_code)]

use bin_data_macros::bin_data;

bin_data! {
    /// some docs
    #[derive(Debug, Copy, Clone)]
    pub struct Test {
        #[bin_data(encode { endian = Endian::Big })]
        some_private_field: i64,
        pub some_pub_field: u8,
        pub(in self) some_fancy_visibility: u32,
        #[bin_data(encode = 42.0)]
        let temporary: f32,
        @magic([0x12, 0x34, 0x56, 0x78]),
    }
}

fn main() {}
