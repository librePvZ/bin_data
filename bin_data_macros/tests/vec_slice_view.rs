use bin_data::data::{Decode, Encode, SliceView};
use bin_data::named_args::Endian;
use bin_data_macros::bin_data;

bin_data! {
    #[derive(Debug, Clone, Eq, PartialEq)]
    #[bin_data(endian = Endian::Little)]
    pub struct Test {
        #[bin_data(encode = values.len() as u32)]
        let length: u32,
        #[bin_data(args:decode { count = length as usize, arg = Endian::Little })]
        #[bin_data(args:encode { arg = Endian::Little })]
        #[bin_data(encode = SliceView::new(values, |&(x, _)| x))]
        let xs: Vec<u8>,
        #[bin_data(args:decode { count = length as usize, arg = Endian::Little })]
        #[bin_data(args:encode { arg = Endian::Little })]
        #[bin_data(encode = SliceView::new(values, |&(_, y)| y))]
        let ys: Vec<u16>,
        #[bin_data(decode = std::iter::zip(xs, ys).collect())]
        pub values: Box<[(u8, u16)]>,
    }
}

fn example() -> Test {
    Test { values: Box::new([(1, 10), (2, 20), (3, 30)]) }
}

#[test]
fn test_decode() {
    let input = [
        3, 0, 0, 0, // length
        1, 2, 3, // xs
        10, 0, 20, 0, 30, 0 // ys
    ];
    let expected = example();
    let decoded = Test::decode(&mut input.as_ref()).unwrap();
    assert_eq!(decoded, expected);
}

#[test]
fn test_encode() {
    let mut output = Vec::new();
    example().encode(&mut output).unwrap();
    let expected = [
        3, 0, 0, 0, // length
        1, 2, 3, // xs
        10, 0, 20, 0, 30, 0 // ys
    ];
    assert_eq!(output, expected);
}

fn main() {}
