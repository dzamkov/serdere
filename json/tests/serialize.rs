use serdere_json::{to_str, TextSerializer, TextSerializerConfig};
use serdere_json::{ValueExt, ValueSerialierExt};
use serdere::{Serialize, Value};
use indoc::*;

#[test]
fn test_bool() {
    assert_eq!(to_str(&true), "true");
    assert_eq!(to_str(&false), "false");
}

#[test]
fn test_str() {
    assert_eq!(to_str("Hello world!"), "\"Hello world!\"");
    assert_eq!(to_str("\t\n"), "\"\\t\\n\"");
}

#[test]
fn test_number() {
    assert_eq!(to_str::<u32>(&1234), "1234");
    assert_eq!(to_str::<i32>(&-1400), "-1400");
    assert_eq!(to_str::<f32>(&1.5), "1.5");
    assert_eq!(to_str::<f32>(&-0.125), "-0.125");
}

#[test]
fn test_tuple() {
    assert_eq!(to_str::<[u32; 3]>(&[3, 6, 9]), "[3, 6, 9]");
    assert_eq!(to_str::<(u32, bool)>(&(3, false)), "[3, false]");
}

#[test]
fn test_option() {
    assert_eq!(
        to_str::<Option<Option<bool>>>(&Some(Some(true))),
        "{ \"has_value\": true, \"value\": true }"
    );
    assert_eq!(
        to_str::<Option<Option<bool>>>(&Some(None)),
        "{ \"has_value\": true, \"value\": null }"
    );
    assert_eq!(
        to_str::<Option<Option<bool>>>(&None),
        "{ \"has_value\": false }"
    );
}

#[test]
fn test_object_simple() {
    let mut res = String::new();
    let mut s = TextSerializer::new(
        TextSerializerConfig {
            indent: Some("    "),
        },
        &mut res,
    );
    let expected = indoc! {
        r#"{
            "name": "Finland",
            "pop": 5500000,
            "langs": [
                "fi",
                "sv"
            ],
            "metadata": {}
        }"#
    };
    Value::with(&mut s, |value| {
        let mut root = value.into_object()?;
        root.entry("name")?.put_str("Finland")?;
        root.entry("pop")?.put_u32(5500000)?;
        let mut langs = root.entry("langs")?.into_list_streaming()?;
        langs.push()?.put_str("fi")?;
        langs.push()?.put_str("sv")?;
        langs.close()?;
        root.entry("metadata")?.into_object()?.close()?;
        root.close()
    })
    .unwrap();
    assert_eq!(res, expected);
}

#[test]
fn test_derive_struct() {
    #[derive(PartialEq, Eq, Debug, Serialize)]
    struct Test<Data> {
        name: String,
        age: u32,
        #[serde(rename = "data")]
        data_xyz: Data,
    }
    assert_eq!(
        to_str::<Test<bool>>(&Test {
            name: "Mike".to_string(),
            age: 28,
            data_xyz: false
        }),
        r#"{ "name": "Mike", "age": 28, "data": false }"#
    );
}

#[test]
fn test_derive_enum_simple() {
    #[derive(Serialize)]
    enum Test {
        OptionA,
        Option2,
        TheThirdOption,
        #[serde(rename = "option_4")]
        OptionIV,
        #[serde(rename = "option_5")]
        OptionV,
    }
    assert_eq!(to_str::<Test>(&Test::OptionA), "\"OptionA\"");
    assert_eq!(to_str::<Test>(&Test::Option2), "\"Option2\"");
    assert_eq!(to_str::<Test>(&Test::TheThirdOption), "\"TheThirdOption\"");
    assert_eq!(to_str::<Test>(&Test::OptionIV), "\"option_4\"");
    assert_eq!(to_str::<Test>(&Test::OptionV), "\"option_5\"");
}

#[test]
fn test_derive_enum_complex() {
    #[derive(Serialize)]
    enum Test<Data> {
        #[serde(rename = "unassigned")]
        Unassigned,
        #[serde(rename = "assigned")]
        Assigned {
            name: String,
            age: u32,
            #[serde(rename = "data")]
            data_xyz: Data,
        },
    }
    assert_eq!(
        to_str::<Test<bool>>(&Test::Unassigned),
        r#"{ "type": "unassigned" }"#
    );
    assert_eq!(
        to_str::<Test<bool>>(&Test::Assigned {
            name: "Mike".to_string(),
            age: 28,
            data_xyz: false
        }),
        r#"{ "type": "assigned", "name": "Mike", "age": 28, "data": false }"#
    );
}

#[test]
fn test_derive_enum_transparent() {
    #[derive(Serialize)]
    struct Circle {
        x: i32,
        y: i32,
        radius: i32,
    }
    #[derive(Serialize)]
    struct Rect {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    }
    #[derive(Serialize)]
    enum Shape {
        #[serde(rename = "other")]
        Other,
        #[serde(rename = "circle", transparent)]
        Circle(Circle),
        #[serde(rename = "rect", transparent)]
        Rect { source: Rect },
    }
    assert_eq!(to_str::<Shape>(&Shape::Other), r#"{ "type": "other" }"#);
    assert_eq!(
        to_str::<Shape>(&Shape::Circle(Circle {
            x: 10,
            y: 20,
            radius: 30
        })),
        r#"{ "type": "circle", "x": 10, "y": 20, "radius": 30 }"#
    );
    assert_eq!(
        to_str::<Shape>(&Shape::Rect {
            source: Rect {
                x: 10,
                y: -20,
                width: 30,
                height: 40
            }
        }),
        r#"{ "type": "rect", "x": 10, "y": -20, "width": 30, "height": 40 }"#
    );
}
