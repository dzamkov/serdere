#![allow(clippy::bool_assert_comparison)]
use serdere_json::{from_str, DeserializeError, TextDeserializerConfig, ValueExt};
use serdere_json::{JsonDeserializer, JsonOutliner, TextDeserializer};
use serdere::{Deserialize, Deserializer, Outliner, Value};

#[test]
fn test_bool() {
    assert_eq!(from_str::<bool>("true").unwrap(), true);
    assert_eq!(from_str::<bool>(" false ").unwrap(), false);
    assert!(from_str::<bool>("null").is_err());
}

#[test]
fn test_str() {
    assert_eq!(
        from_str::<String>("\"Hello world!\"").unwrap(),
        "Hello world!"
    );
    assert_eq!(from_str::<String>("\"\\t\\n\"").unwrap(), "\t\n");
}

#[test]
fn test_number() {
    assert_eq!(from_str::<u32>("1234").unwrap(), 1234);
    assert_eq!(from_str::<u32>("-0").unwrap(), 0);
    assert!(from_str::<u32>("-20").is_err());
    assert_eq!(from_str::<u8>("255").unwrap(), 255);
    assert_eq!(from_str::<i8>("127").unwrap(), 127);
    assert_eq!(from_str::<i8>("-128").unwrap(), -128);
    assert!(from_str::<i8>("128").is_err());
    assert_eq!(from_str::<i32>("1400").unwrap(), 1400);
    assert_eq!(from_str::<u32>("12E3").unwrap(), 12000);
    assert_eq!(from_str::<u32>("1000E-3").unwrap(), 1);
    assert!(from_str::<i32>("15.7").is_err());
    assert!(from_str::<u32>("01234").is_err());
    assert!(from_str::<u32>("-01234").is_err());
    assert_eq!(from_str::<f32>("3.125").unwrap(), 3.125);
    assert_eq!(from_str::<f32>("1.0e7").unwrap(), 1.0e7);
    assert_eq!(from_str::<f32>("1.625e+3").unwrap(), 1.625e3);
    assert_eq!(from_str::<f32>("-1.0e-4").unwrap(), -1.0e-4);
    assert_eq!(from_str::<f32>("-0.125").unwrap(), -0.125);
    assert!(from_str::<f32>("-0e5").unwrap().is_sign_negative());
}

#[test]
fn test_option() {
    assert_eq!(
        from_str::<Option<Option<bool>>>("{ \"has_value\": true, \"value\": true }").unwrap(),
        Some(Some(true))
    );
    assert_eq!(
        from_str::<Option<Option<bool>>>("{ \"has_value\": true, \"value\": null }").unwrap(),
        Some(None)
    );
    assert_eq!(
        from_str::<Option<Option<bool>>>("{ \"has_value\": false }").unwrap(),
        None
    );
}

#[test]
fn test_enum() {
    use serdere::{Deserialize, FixedNameMap, NameMap};
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    enum TestEnum {
        Alpha,
        Beta,
        Gamma,
    }
    impl<D: Deserializer + ?Sized, Ctx: ?Sized> Deserialize<D, Ctx> for TestEnum {
        const NULLABLE: bool = false;
        fn deserialize(value: Value<D>, _: &mut Ctx) -> Result<Self, D::Error> {
            const VALUES: &[TestEnum] = &[TestEnum::Alpha, TestEnum::Beta, TestEnum::Gamma];
            const NAMES: &NameMap<usize> =
                FixedNameMap::new([("alpha", 0), ("beta", 1), ("gamma", 2)]).unfix();
            Ok(VALUES[value.get_tag(2, NAMES)?])
        }
    }
    assert_eq!(from_str::<TestEnum>("\"alpha\"").unwrap(), TestEnum::Alpha);
    assert_eq!(from_str::<TestEnum>("\"gamma\"").unwrap(), TestEnum::Gamma);
    assert_eq!(from_str::<TestEnum>("1").unwrap(), TestEnum::Beta);
    assert!(from_str::<TestEnum>("\"delta\"").is_err());
    assert!(from_str::<TestEnum>("3").is_err());
}

#[test]
fn test_list() {
    assert_eq!(
        from_str::<Vec<u32>>("[1, 1, 2, 3, 5, 7]").unwrap(),
        vec![1, 1, 2, 3, 5, 7]
    );
}

#[test]
fn test_tuple() {
    assert_eq!(from_str::<[u32; 3]>("[3, 6, 9]").unwrap(), [3, 6, 9]);
    assert_eq!(from_str::<(u32, bool)>("[3, false]").unwrap(), (3, false));
    assert!(from_str::<[u32; 4]>("[3, 6, 9]").is_err());
}

#[test]
fn test_object_simple() {
    let source = r#"{
        "name": "Finland",
        /* The population of the country */
        "pop": 5.5e6,
        // The official languages of the country
        "langs": ["fi", "sv"]
    }"#;
    let mut d = TextDeserializer::new(
        TextDeserializerConfig {
            allow_comments: true,
        },
        source,
    )
    .unwrap();
    let res = Value::with(&mut d, |value| {
        let mut root = value.into_object()?;
        assert_eq!(root.entry("name")?.get_str()?, "Finland");
        assert_eq!(root.entry("pop")?.get_u32()?, 5500000);
        let mut langs = root.entry("langs")?.into_list()?;
        assert_eq!(langs.next()?.unwrap().get_str()?, "fi");
        assert_eq!(langs.next()?.unwrap().get_str()?, "sv");
        assert!(langs.next()?.is_none());
        root.close()
    });
    res.unwrap();
}

#[test]
fn test_lookback() {
    let source = r#"{
        "bool_true": true,
        "bool_false": false,
        "list": [1, 2, 3],
        "empty_list": [],
        "string": "test",
        "empty_object": {},
        "null": null,
        "last": "entry"
    }"#;
    let mut d = TextDeserializer::new(Default::default(), source).unwrap();
    let res: Result<(), DeserializeError<_>> = (|| {
        d.open_object()?;
        d.try_push_entry("last")?;
        assert_eq!(d.read_str()?, "entry");
        d.try_push_entry("bool_false")?;
        assert_eq!(d.get_bool()?, false);
        d.try_push_entry("bool_true")?;
        assert_eq!(d.get_bool()?, true);
        d.try_push_entry("list")?;
        d.open_list()?;
        assert_eq!(d.next_item()?, true);
        assert_eq!(d.get_u32()?, 1);
        assert_eq!(d.next_item()?, true);
        assert_eq!(d.get_u32()?, 2);
        assert_eq!(d.next_item()?, true);
        assert_eq!(d.get_u32()?, 3);
        assert_eq!(d.next_item()?, false);
        d.try_push_entry("empty_list")?;
        d.open_list()?;
        assert_eq!(d.next_item()?, false);
        d.try_push_entry("empty_object")?;
        d.open_object()?;
        assert_eq!(d.next_entry()?, false);
        d.try_push_entry("null")?;
        d.pop_null()?;
        d.try_push_entry("string")?;
        assert_eq!(d.read_str()?, "test");
        d.close_object()?;
        d.close()?;
        Ok(())
    })();
    res.unwrap();
}

#[test]
fn test_object_out_of_order() {
    let source = r#"{
        "del\ta": 4,
        "beta": 2,
        "alphaplus": 10,
        "alp": 20,
        "alpha": 1,
        "gamma": 3
    }"#;
    let mut d = TextDeserializer::new(Default::default(), source).unwrap();
    let res: Result<(), DeserializeError<_>> = (|| {
        d.open_object()?;
        assert!(d.try_push_entry("alpha")?);
        assert_eq!(d.get_u32()?, 1);
        assert!(d.try_push_entry("beta")?);
        assert_eq!(d.get_u32()?, 2);
        assert!(d.try_push_entry("gamma")?);
        assert_eq!(d.get_u32()?, 3);
        assert!(d.try_push_entry("del\ta")?);
        assert_eq!(d.get_u32()?, 4);
        assert!(d.try_push_entry("alphaplus")?);
        assert_eq!(d.get_u32()?, 10);
        assert!(d.try_push_entry("alp")?);
        assert_eq!(d.get_u32()?, 20);
        d.close_object()?;
        d.close()?;
        Ok(())
    })();
    res.unwrap();
}

#[test]
fn test_object_complex() {
    let source = r#"{
        "animal": {
            "tetrapod": {
                "mammal": "goat",
                "reptile": "lizard",
                "bird": "sparrow"
            },
            "crustacean": {
                "crab": {}
            }
        },
        "plant": {
            "bryophyte": "moss",
            "spermatophyte": {
                "conifer": {
                    "pinus": "pine"
                }
            }
        },
        "fungus": {}
    }"#;
    let mut d = TextDeserializer::new(Default::default(), source).unwrap();
    let res = Value::with(&mut d, |value| {
        let mut root = value.into_object()?;
        let mut plant = root.entry("plant")?.into_object()?;
        let mut spermatophyte = plant.entry("spermatophyte")?.into_object()?;
        let mut conifer = spermatophyte.entry("conifer")?.into_object()?;
        assert_eq!(conifer.entry("pinus")?.get_str()?, "pine");
        conifer.close()?;
        spermatophyte.close()?;
        assert_eq!(plant.entry("bryophyte")?.get_str()?, "moss");
        plant.close()?;
        let mut animal = root.entry("animal")?.into_object()?;
        let mut crustacean = animal.entry("crustacean")?.into_object()?;
        let crab = crustacean.entry("crab")?.into_object()?;
        crab.close()?;
        crustacean.close()?;
        let mut tetrapod = animal.entry("tetrapod")?.into_object()?;
        assert_eq!(tetrapod.entry("mammal")?.get_str()?, "goat");
        assert_eq!(tetrapod.entry("reptile")?.get_str()?, "lizard");
        assert_eq!(tetrapod.entry("bird")?.get_str()?, "sparrow");
        tetrapod.close()?;
        animal.close()?;
        let fungus = root.entry("fungus")?.into_object()?;
        fungus.close()?;
        root.close()
    });
    res.unwrap();
}

#[test]
fn test_object_next_entry() {
    let source = r#"{
        "asdf": "fdsa",
        "hello": "olleh",
        "world": "dlrow",
        "hjkl": "lkjh"
    }"#;
    let mut d = TextDeserializer::new(Default::default(), source).unwrap();
    let res: Result<(), DeserializeError<_>> = (|| {
        d.open_object()?;
        d.try_push_entry("world")?;
        assert_eq!(d.read_str()?, "dlrow");
        while d.next_entry()? {
            let key = d.flush_str()?.into_owned();
            let value = d.read_str()?;
            assert_eq!(key.chars().rev().collect::<String>(), value);
        }
        d.close()?;
        Ok(())
    })();
    res.unwrap();
}

#[test]
fn test_derive_struct() {
    #[derive(PartialEq, Eq, Debug, Deserialize)]
    struct Test<Data> {
        name: String,
        #[serde(proxy = u32)]
        age: u64,
        #[serde(rename = "data")]
        data_xyz: Data,
        #[serde(default)]
        other_info: Vec<String>,
    }
    let source = r#"{
        "name": "Mike",
        "age": 28,
        "data": false
    }"#;
    assert_eq!(
        from_str::<Test<bool>>(source).unwrap(),
        Test {
            name: "Mike".to_string(),
            age: 28,
            data_xyz: false,
            other_info: Vec::new()
        }
    );
}

#[test]
fn test_derive_enum_simple() {
    #[derive(PartialEq, Eq, Debug, Deserialize)]
    enum Test {
        OptionA,
        Option2,
        TheThirdOption,
        #[serde(rename = "option_4")]
        OptionIV,
        #[serde(rename = "option_5")]
        OptionV,
    }
    assert_eq!(from_str::<Test>("\"OptionA\"").unwrap(), Test::OptionA);
    assert_eq!(from_str::<Test>("\"Option2\"").unwrap(), Test::Option2);
    assert_eq!(
        from_str::<Test>("\"TheThirdOption\"").unwrap(),
        Test::TheThirdOption
    );
    assert_eq!(from_str::<Test>("\"option_4\"").unwrap(), Test::OptionIV);
    assert_eq!(from_str::<Test>("\"option_5\"").unwrap(), Test::OptionV);
    assert!(from_str::<Test>("\"OptionIV\"").is_err());
    assert!(from_str::<Test>("\"OptionV\"").is_err());
}

#[test]
fn test_derive_enum_indices() {
    #[derive(PartialEq, Eq, Debug, Deserialize)]
    enum Test {
        A,
        B = 3,
        C,
        #[serde(reindex = 10)]
        D,
        E,
        F = 9
    }
    assert_eq!(from_str::<Test>("0").unwrap(), Test::A);
    assert_eq!(from_str::<Test>("3").unwrap(), Test::B);
    assert_eq!(from_str::<Test>("4").unwrap(), Test::C);
    assert_eq!(from_str::<Test>("10").unwrap(), Test::D);
    assert_eq!(from_str::<Test>("6").unwrap(), Test::E);
    assert_eq!(from_str::<Test>("9").unwrap(), Test::F);
    assert!(from_str::<Test>("5").is_err());
}

#[test]
fn test_derive_enum_complex() {
    #[derive(PartialEq, Eq, Debug, Deserialize)]
    enum Test<Data> {
        #[serde(rename = "unassigned")]
        Unassigned,
        #[serde(rename = "assigned")]
        #[serde(reindex = 3)]
        Assigned {
            name: String,
            age: u32,
            #[serde(rename = "data")]
            data_xyz: Data,
        },
    }
    let source = r#"{
        "type": "assigned",
        "name": "Mark",
        "age": 23,
        "data": true
    }"#;
    assert_eq!(
        from_str::<Test<bool>>(source).unwrap(),
        Test::Assigned {
            name: "Mark".to_string(),
            age: 23,
            data_xyz: true
        }
    );
    let source = r#"{
        "type": "unassigned"
    }"#;
    assert_eq!(from_str::<Test<bool>>(source).unwrap(), Test::Unassigned);
    assert!(from_str::<Test<bool>>("{ \"type\": 2 }").is_err());
}

#[test]
fn test_derive_enum_transparent() {
    #[derive(PartialEq, Eq, Debug, Deserialize)]
    struct Circle {
        x: i32,
        y: i32,
        radius: i32,
    }
    #[derive(PartialEq, Eq, Debug, Deserialize)]
    struct Rect {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    }
    #[derive(PartialEq, Eq, Debug, Deserialize)]
    enum Shape {
        #[serde(rename = "other")]
        Other,
        #[serde(rename = "circle", transparent)]
        Circle(Circle),
        #[serde(rename = "rect", transparent)]
        Rect { source: Rect },
    }
    let source = r#"{
        "type": "circle",
        "x": 10,
        "y": 20,
        "radius": 30
    }"#;
    assert_eq!(
        from_str::<Shape>(source).unwrap(),
        Shape::Circle(Circle {
            x: 10,
            y: 20,
            radius: 30
        })
    );
    let source = r#"{
        "type": "rect",
        "x": 10,
        "y": -20,
        "width": 30,
        "height": 40
    }"#;
    assert_eq!(
        from_str::<Shape>(source).unwrap(),
        Shape::Rect {
            source: Rect {
                x: 10,
                y: -20,
                width: 30,
                height: 40
            }
        }
    );
}
