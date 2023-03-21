use bin_data_macros::bin_data;

bin_data! {
    #[bin_data(endian = "little")]
    pub struct MissingEncode {
        let temporary: u32,
        #[bin_data(decode = temporary)]
        pub field: u32,
    }
}

fn main() {}
