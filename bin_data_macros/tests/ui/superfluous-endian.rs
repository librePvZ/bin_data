use bin_data_macros::bin_data;

bin_data! {
    #[bin_data(endian = "inherit")]
    pub struct SuperfluousEndian {
        #[bin_data(endian = "little")]
        cannot_be_little: (),
        #[bin_data(endian = "big")]
        cannot_be_big: (),
        #[bin_data(endian = "inherit")]
        cannot_inherit: (),
        #[bin_data(endian = "none")]
        explicit_none_okay: (),
        implicit_inherit_okay: (),
    }
}

fn main() {}
