use bin_data_macros::bin_data;
use bin_data::named_args::Endian;

bin_data! {
    #[bin_data(endian = Endian::Little)]
    pub struct MissingEndian {
        let temporary: u32,
        #[bin_data(decode = temporary)]
        pub field: u32,
    }
}

fn main() {}
