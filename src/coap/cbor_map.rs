
pub struct CborMap {
    map: Vec<(ciborium::value::Value, ciborium::value::Value)>,
}

impl CborMap {
    /*
    pub fn new() -> Self {
        Self {
            map: Vec::new(),
        }
    }
    */

    pub fn from_slice(slice: &[(&str, ciborium::value::Value)]) -> Self {
        Self {
            map: slice.iter()
                .map(|p| Self::key_val_pair_to_entry(p.0, p.1.clone())) // TODO: Do not clone? Is it possible to move out of slice?
                .collect(),
        }
    }

    pub fn as_ciborium_map(self) -> ciborium::value::Value {
        ciborium::value::Value::Map(self.map)
    }

    /*
    pub fn push(&mut self, key: &str, value: ciborium::value::Value) {
        self.map.push(Self::key_val_pair_to_entry(key, value));
    }
    */

    fn key_val_pair_to_entry(key: &str, value: ciborium::value::Value) -> (ciborium::value::Value, ciborium::value::Value) {
        (ciborium::value::Value::Text(key.to_string()), value)
    }
}
