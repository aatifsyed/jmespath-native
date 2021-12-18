use recap::Recap;
use serde::Deserialize;
use serde_json::Value::{self, Array, Bool, Null, Number, Object, String};
use std::ops;

#[derive(Debug, Default, Deserialize, Recap, PartialEq, Eq, Hash, Clone, Copy)]
#[recap(regex = r#"^(?P<start>-?\d+)?:(?P<end>-?\d+)?(:(?P<step>-?\d+)?)?$"#)]
pub struct JMESSlice {
    pub start: Option<isize>,
    pub end: Option<isize>,
    pub step: Option<isize>,
}

impl From<ops::Range<isize>> for JMESSlice {
    fn from(range: ops::Range<isize>) -> Self {
        Self {
            start: Some(range.start),
            end: Some(range.end),
            step: None,
        }
    }
}
impl From<ops::RangeFrom<isize>> for JMESSlice {
    fn from(range: ops::RangeFrom<isize>) -> Self {
        Self {
            start: Some(range.start),
            ..Default::default()
        }
    }
}
impl From<ops::RangeTo<isize>> for JMESSlice {
    fn from(range: ops::RangeTo<isize>) -> Self {
        Self {
            end: Some(range.end),
            ..Default::default()
        }
    }
}

pub trait JMESPath {
    fn identify(self, key: impl AsRef<str>) -> Self;
    fn index(self, index: isize) -> Self;
    fn slice(self, slice: impl Into<JMESSlice>) -> Self;
}

/// If index is negative, calculate the index from the back of the vec
/// Bail if index is too negative
macro_rules! index_from_rear {
    ($vec:expr, $index:expr) => {{
        if $index.is_negative() {
            // Get the index from the back
            match $vec.len().checked_sub($index.unsigned_abs()) {
                Some(u) => u,
                None => return Null,
            }
        } else {
            $index.unsigned_abs() // Positive or 0
        }
    }};
}

impl JMESPath for Value {
    fn identify(self, key: impl AsRef<str>) -> Self {
        match self {
            Object(mut map) => map.remove(key.as_ref()).unwrap_or(Null),
            _ => Null,
        }
    }

    fn index(self, index: isize) -> Self {
        match self {
            Array(mut vec) => {
                let index = index_from_rear!(vec, index);
                if index < vec.len() {
                    vec.remove(index)
                } else {
                    Null // OOB
                }
            }
            _ => Null,
        }
    }

    fn slice(self, slice: impl Into<JMESSlice>) -> Self {
        match self {
            Array(vec) => {
                let slice: JMESSlice = slice.into();
                let start = match slice.start {
                    Some(index) => index_from_rear!(vec, index),
                    None => todo!(),
                };
                todo!()
            }
            _ => Null,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn flatmap() -> Value {
        json!({"a": "foo", "b": "bar", "c": "baz"})
    }
    fn nested_map() -> Value {
        json!({"a": {"b": {"c": {"d": "value"}}}})
    }
    fn array() -> Value {
        json!(["a", "b", "c", "d", "e", "f"])
    }
    fn complex() -> Value {
        json!({"a": {
          "b": {
            "c": [
              {"d": [0, [1, 2]]},
              {"d": [3, 4]}
            ]
          }
        }})
    }

    #[test]
    fn identifier() {
        assert_eq!(flatmap().identify("a"), json!("foo"));
        assert_eq!(flatmap().identify("d"), json!(null));
        assert_eq!(
            nested_map()
                .identify("a")
                .identify("b")
                .identify("c")
                .identify("d"),
            json!("value")
        )
    }

    #[test]
    fn index() {
        assert_eq!(array().index(1), json!("b"));
        assert_eq!(array().index(-1), json!("f"));
        assert_eq!(array().index(10), json!(null));
        assert_eq!(array().index(-10), json!(null));
    }

    #[test]
    fn combined() {
        assert_eq!(
            complex()
                .identify("a")
                .identify("b")
                .identify("c")
                .index(0)
                .identify("d")
                .index(1)
                .index(0),
            json!(1)
        )
    }
    #[test]
    fn parse_jmes_slice() {
        let res = "::".parse::<JMESSlice>();
        assert_eq!(res, Ok(JMESSlice::default()));
        let res = "0:1".parse::<JMESSlice>();
        assert_eq!(res, Ok((0..1).into()));
        let res = "-10:".parse::<JMESSlice>();
        assert_eq!(res, Ok((-10..).into()));
        let res = ":100".parse::<JMESSlice>();
        assert_eq!(res, Ok((..100).into()));
        let res = "::10".parse::<JMESSlice>();
        assert_eq!(
            res,
            Ok(JMESSlice {
                start: None,
                end: None,
                step: Some(10)
            })
        );
    }
}
