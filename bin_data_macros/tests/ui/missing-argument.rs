use bin_data_macros::bin_data;

bin_data! {
    pub struct MissingEndian {
        this_field_needs_an_endian: u32,
    }
}

fn main() {}
