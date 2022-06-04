use rust_decimal::prelude::*;

pub struct CborParser();

impl CborParser {
    pub fn to_decimal(value: &ciborium::value::Value) -> Result<Decimal, String> {
        match value {
            ciborium::value::Value::Tag(4, value) => {
                match &**value {
                    ciborium::value::Value::Array(vec) => {
                        if vec.len() == 2 {
                            if let (ciborium::value::Value::Integer(scale), ciborium::value::Value::Integer(num)) = (&vec[0], &vec[1]) {
                                let num: i128 = (*num).try_into().unwrap();
                                let scale: i128 = (*scale).try_into().unwrap();
                                Ok(Decimal::new(num.try_into().unwrap(), (-scale).try_into().unwrap()))
                            } else {
                                Err("Unexpected type in decimal array".to_string())
                            }
                        } else {
                            Err("Unexpcected number of entries in decimal array".to_string())
                        }
                    },
                    _ => Err("Unexpected type in decimal tag".to_string())
                }
            },
            _ => Err("Unknown cbor type".to_string())
        }
    }

    pub fn from_decimal(value: &Decimal) -> Result<ciborium::value::Value, std::num::TryFromIntError> {
        use ciborium::value::{Value, Integer};

        Ok(Value::Tag(4, Box::new(Value::Array(
                [Value::Integer(Integer::from(-i32::try_from(value.scale())?)),
                 Value::Integer(Integer::from(i64::try_from(value.mantissa())?))
                ].to_vec()
        ))))
    }
}
