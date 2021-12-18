use num::Zero;
use serde_json::Value::{self, Array, Bool, Null, Number, Object, String};
use std::{num::NonZeroIsize, ops, str};
use thiserror::Error;

#[derive(Debug, Default, PartialEq, Eq, Hash, Clone, Copy)]
pub struct JMESSlice {
    pub start: Option<isize>,
    pub end: Option<isize>,
    pub step: Option<NonZeroIsize>,
}

#[derive(Debug, Error, PartialEq, Eq, Hash, Clone, Copy)]
pub enum ParseJMESSliceError {
    #[error("Invalid format")]
    InvalidFormat,
    #[error("Step not allowed to be Zero")]
    StepNotAllowedToBeZero,
}

impl str::FromStr for JMESSlice {
    type Err = ParseJMESSliceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use ParseJMESSliceError::{InvalidFormat, StepNotAllowedToBeZero};
        let (_whole, start, end, _colon, step) = lazy_regex::regex_captures!(
            r"^(?P<start>-?\d+)?:(?P<end>-?\d+)?(:(?P<step>-?\d+)?)?$",
            s
        )
        .ok_or(InvalidFormat)?;
        let option_isize = |s| match s {
            "" => None,
            s => Some(s.parse::<isize>().expect("Regex ensures valid")),
        };
        let ok = Self {
            start: option_isize(start),
            end: option_isize(end),
            step: match option_isize(step) {
                Some(i) => Some(NonZeroIsize::new(i).ok_or(StepNotAllowedToBeZero)?),
                None => None,
            },
        };
        Ok(ok)
    }
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

pub trait JMESPath: Sized {
    fn identify(self, key: impl AsRef<str>) -> Self;
    fn index(self, index: isize) -> Self;
    fn slice(self, slice: impl Into<JMESSlice>) -> Self;
    fn list_project(self, projection: impl Fn(Self) -> Self) -> Self;
    fn slice_project(self, slice: impl Into<JMESSlice>, projection: impl Fn(Self) -> Self) -> Self;
    fn object_project(self, projection: impl Fn(Self) -> Self) -> Self;
    fn flatten(self) -> Self;
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
                let index = if index.is_negative() {
                    // Get the index from the back
                    match vec.len().checked_sub(index.unsigned_abs()) {
                        Some(u) => u,
                        None => return Null,
                    }
                } else {
                    index.unsigned_abs()
                };
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
        use slyce::{Index, Slice}; // Slicing makes my head hurt, use a library
        let slice: JMESSlice = slice.into();
        match self {
            Array(vec) => {
                let op = Slice {
                    start: match slice.start {
                        Some(i) if i.is_negative() => Index::Tail(i.unsigned_abs()),
                        Some(i) => Index::Head(i.unsigned_abs()),
                        None => Index::Default,
                    },
                    end: match slice.end {
                        Some(i) if i.is_negative() => Index::Tail(i.unsigned_abs()),
                        Some(i) => Index::Head(i.unsigned_abs()),
                        None => Index::Default,
                    },
                    step: slice.step.map(isize::from),
                };
                Array(op.apply(&vec).map(Clone::clone).collect())
            }
            _ => Null,
        }
    }

    fn list_project(self, projection: impl Fn(Self) -> Self) -> Self {
        match self {
            Array(vec) => Array(
                vec.into_iter()
                    .map(projection)
                    .filter(|value| !value.is_null())
                    .collect(),
            ),
            _ => Null,
        }
    }

    fn slice_project(self, slice: impl Into<JMESSlice>, projection: impl Fn(Self) -> Self) -> Self {
        match self {
            Array(_) => self.slice(slice).list_project(projection),
            _ => Null,
        }
    }

    fn object_project(self, projection: impl Fn(Self) -> Self) -> Self {
        match self {
            Object(map) => Array(
                map.into_iter()
                    .map(|(_key, value)| value)
                    .map(projection)
                    .filter(|value| !value.is_null())
                    .collect(),
            ),
            _ => Null,
        }
    }

    fn flatten(self) -> Self {
        match self {
            Array(vec) => {
                let mut results = Vec::new();
                for result in vec.into_iter() {
                    match result {
                        Array(mut inner) => results.append(&mut inner),
                        other => results.push(other),
                    }
                }
                Array(results)
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
    fn array() -> Value {
        json!(["a", "b", "c", "d", "e", "f"])
    }

    #[test]
    fn index() {
        assert_eq!(array().index(1), json!("b"));
        assert_eq!(array().index(-1), json!("f"));
        assert_eq!(array().index(10), json!(null));
        assert_eq!(array().index(-10), json!(null));
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
                step: Some(NonZeroIsize::new(10).unwrap())
            })
        );
        let res = "::0".parse::<JMESSlice>();
        assert_eq!(res, Err(ParseJMESSliceError::StepNotAllowedToBeZero));
    }

    fn slice_example() -> Value {
        json!([0, 1, 2, 3])
    }
    #[test]
    fn slicing() -> anyhow::Result<()> {
        assert_eq!(
            slice_example().slice("0:4:1".parse::<JMESSlice>()?),
            json!([0, 1, 2, 3])
        );
        assert_eq!(
            slice_example().slice("0:4".parse::<JMESSlice>()?),
            json!([0, 1, 2, 3])
        );
        assert_eq!(
            slice_example().slice("0:3".parse::<JMESSlice>()?),
            json!([0, 1, 2])
        );
        assert_eq!(
            slice_example().slice(":2".parse::<JMESSlice>()?),
            json!([0, 1])
        );
        assert_eq!(
            slice_example().slice("::2".parse::<JMESSlice>()?),
            json!([0, 2])
        );
        assert_eq!(
            slice_example().slice("::-1".parse::<JMESSlice>()?),
            json!([3, 2, 1, 0]),
        );
        assert_eq!(
            slice_example().slice("-2:".parse::<JMESSlice>()?),
            json!([2, 3])
        );
        assert_eq!(
            slice_example().slice("100::-1".parse::<JMESSlice>()?),
            json!([3, 2, 1, 0])
        );
        Ok(())
    }

    fn list_project_example() -> Value {
        json!({
          "people": [
            {"first": "James", "last": "d"},
            {"first": "Jacob", "last": "e"},
            {"first": "Jayden", "last": "f"},
            {"missing": "different"}
          ],
          "foo": {"bar": "baz"}
        })
    }

    #[test]
    fn list_projection() {
        assert_eq!(
            list_project_example()
                .identify("people")
                .list_project(|v| v.identify("first")),
            json!(["James", "Jacob", "Jayden"])
        );
    }

    #[test]
    fn slice_projection() {
        assert_eq!(
            list_project_example()
                .identify("people")
                .slice_project(":2".parse::<JMESSlice>().unwrap(), |v| v.identify("first")),
            json!(["James", "Jacob"])
        );
    }

    fn object_projection_example() -> Value {
        json!({
          "ops": {
            "functionA": {"numArgs": 2},
            "functionB": {"numArgs": 3},
            "functionC": {"variadic": true}
          }
        })
    }

    #[test]
    fn object_projection() {
        assert_eq!(
            object_projection_example()
                .identify("ops")
                .object_project(|v| v.identify("numArgs")),
            json!([2, 3])
        )
    }

    fn flatten_projection_example() -> Value {
        json!({
          "reservations": [
            {
              "instances": [
                {"state": "running"},
                {"state": "stopped"}
              ]
            },
            {
              "instances": [
                {"state": "terminated"},
                {"state": "running"}
              ]
            }
          ]
        })
    }

    #[test]
    fn flatten_projection() {
        assert_eq!(
            flatten_projection_example()
                .identify("reservations")
                .list_project(|v| v
                    .identify("instances")
                    .list_project(|v| v.identify("state"))),
            json!([["running", "stopped"], ["terminated", "running"]])
        );
        assert_eq!(
            flatten_projection_example()
                .identify("reservations")
                .flatten()
                .list_project(|v| v.identify("instances")),
            json!(["running", "stopped", "terminated", "running"]),
            "TODO sound flattening"
        );
    }

    fn nested_list_example() -> Value {
        json!([[0, 1], 2, [3], 4, [5, [6, 7]]])
    }

    #[test]
    fn flatten_project_nested_list() {
        assert_eq!(
            nested_list_example().flatten(),
            json!([0, 1, 2, 3, 4, 5, [6, 7]])
        );
        assert_eq!(
            nested_list_example().flatten().flatten(),
            json!([0, 1, 2, 3, 4, 5, 6, 7]),
        )
    }
}
