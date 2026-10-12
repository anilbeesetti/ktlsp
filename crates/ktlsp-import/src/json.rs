use crate::{ImportError, MAX_DEPTH, MAX_NODES};
use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use std::fmt;

struct Scan<'a> {
    nodes: &'a mut usize,
    depth: usize,
    exceeded: &'a mut Option<&'static str>,
}
impl<'de> DeserializeSeed<'de> for Scan<'_> {
    type Value = Value;
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<Value, D::Error> {
        *self.nodes += 1;
        let budget = if self.depth > MAX_DEPTH {
            Some("depth")
        } else if *self.nodes > MAX_NODES {
            Some("nodes")
        } else {
            None
        };
        if let Some(budget) = budget {
            *self.exceeded = Some(budget);
            return Err(serde::de::Error::custom("workspace JSON budget exceeded"));
        }
        d.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Scan<'_> {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded workspace JSON")
    }
    fn visit_bool<E>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }
    fn visit_i64<E>(self, v: i64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }
    fn visit_u64<E>(self, v: u64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }
    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Value, E> {
        Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite number"))
    }
    fn visit_str<E>(self, v: &str) -> Result<Value, E> {
        Ok(Value::String(v.into()))
    }
    fn visit_string<E>(self, v: String) -> Result<Value, E> {
        Ok(Value::String(v))
    }
    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = a.next_element_seed(Scan {
            nodes: self.nodes,
            depth: self.depth + 1,
            exceeded: self.exceeded,
        })? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Value, A::Error> {
        let mut values = Map::new();
        while let Some(key) = a.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(serde::de::Error::custom(format!(
                    "duplicate JSON key {key:?}"
                )));
            }
            let value = a.next_value_seed(Scan {
                nodes: self.nodes,
                depth: self.depth + 1,
                exceeded: self.exceeded,
            })?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}
pub(crate) fn parse(bytes: &[u8]) -> Result<Value, ImportError> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let mut exceeded = None;
    let result = Scan {
        nodes: &mut 0,
        depth: 1,
        exceeded: &mut exceeded,
    }
    .deserialize(&mut deserializer);
    if let Some(budget) = exceeded {
        return Err(ImportError::BudgetExceeded(budget));
    }
    let value = result.map_err(ImportError::Json)?;
    deserializer.end().map_err(ImportError::Json)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_node_and_depth_bounds() {
        let nodes = format!("[{}]", vec!["null"; MAX_NODES - 1].join(","));
        assert!(parse(nodes.as_bytes()).is_ok());
        let nodes = format!("[{}]", vec!["null"; MAX_NODES].join(","));
        assert!(matches!(
            parse(nodes.as_bytes()),
            Err(ImportError::BudgetExceeded("nodes"))
        ));
        let depth = format!(
            "{}null{}",
            "[".repeat(MAX_DEPTH - 1),
            "]".repeat(MAX_DEPTH - 1)
        );
        assert!(parse(depth.as_bytes()).is_ok());
        let depth = format!("[{depth}]");
        assert!(matches!(
            parse(depth.as_bytes()),
            Err(ImportError::BudgetExceeded("depth"))
        ));
    }
}
