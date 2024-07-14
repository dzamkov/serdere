use serdere::json::{to_str, from_str};

#[test]
#[ignore]
fn test_f32_roundtrip_exhaustive() {
    for i in 0..u32::MAX {
        let exp = f32::from_bits(i);
        if exp.is_finite() {
            let s = to_str(&exp);
            let act = from_str(&s).unwrap();
            assert_eq!(exp, act);
        }
        if i % 65536 == 0 {
            println!("Testing float: {} ({})", i, exp);
        }
    }
}