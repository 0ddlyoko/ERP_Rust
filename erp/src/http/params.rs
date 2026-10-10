use crate::Result;
use crate::environment::Environment;
use crate::model::Model;
use erp_search_code_gen::make_domain;
use erp_types::field::SingleId;
use erp_types::model::CommonModel;
use std::error::Error;
use std::fmt;

/// A value a controller method can take from a request parameter.
///
/// `None` is a parameter that was not sent. Only `Option<T>` accepts that; every other type
/// refuses it, so a missing parameter is a caller's mistake rather than a silent default.
///
/// Takes the environment because a record is read from the database, with the caller's rights.
/// A [`ParamError`] is the caller's mistake, answered with the parameter's name; any other error —
/// a refused read, a database failure — is answered like one the controller raised itself.
pub trait FromParam: Sized {
    fn from_param(env: &mut Environment, raw: Option<String>) -> Result<Self>;
}

/// Why a parameter could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParamError {
    Missing,
    Invalid { expected: &'static str },
    NoSuchRecord,
}

impl fmt::Display for ParamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParamError::Missing => f.write_str("it is missing"),
            ParamError::Invalid { expected } => write!(f, "it is not {expected}"),
            ParamError::NoSuchRecord => f.write_str("it names no record"),
        }
    }
}

impl Error for ParamError {}

impl FromParam for String {
    fn from_param(_env: &mut Environment, raw: Option<String>) -> Result<Self> {
        Ok(raw.ok_or(ParamError::Missing)?)
    }
}

impl FromParam for bool {
    fn from_param(_env: &mut Environment, raw: Option<String>) -> Result<Self> {
        match raw.ok_or(ParamError::Missing)?.as_str() {
            "1" | "true" | "True" | "on" => Ok(true),
            "0" | "false" | "False" | "off" => Ok(false),
            _ => Err(ParamError::Invalid {
                expected: "a boolean",
            }
            .into()),
        }
    }
}

macro_rules! from_param_parsed {
    ($($ty:ty => $expected:literal),* $(,)?) => {
        $(
            impl FromParam for $ty {
                fn from_param(_env: &mut Environment, raw: Option<String>) -> Result<Self> {
                    Ok(raw.ok_or(ParamError::Missing)?
                        .parse()
                        .map_err(|_| ParamError::Invalid { expected: $expected })?)
                }
            }
        )*
    };
}

from_param_parsed!(
    i32 => "a whole number",
    i64 => "a whole number",
    u32 => "a positive whole number",
    u64 => "a positive whole number",
    f64 => "a number",
);

/// Absent when the parameter was not sent, and when it names no record: an optional record the
/// URL points past is simply not there. A value that cannot be read at all is still refused.
impl<T: FromParam> FromParam for Option<T> {
    fn from_param(env: &mut Environment, raw: Option<String>) -> Result<Self> {
        let Some(raw) = raw else {
            return Ok(None);
        };
        match T::from_param(env, Some(raw)) {
            Ok(value) => Ok(Some(value)),
            Err(error) if matches!(error.downcast_ref(), Some(ParamError::NoSuchRecord)) => {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }
}

/// The record an id names, as the caller sees it.
///
/// A record the caller may not see is answered as missing, like one that does not exist: telling
/// them apart would say which ids exist. A model the caller may not read at all is refused as
/// such, since that says nothing about any record.
///
/// What `#[derive(Model)]` generates for every model, so a controller argument typed as a record
/// is read from its id.
pub fn find_record<M: Model<SingleId>>(env: &mut Environment, raw: Option<String>) -> Result<M> {
    let id = u32::from_param(env, raw)?;
    let found = env.search_ids(
        <M as CommonModel<SingleId>>::_get_model_name(),
        &make_domain!([("id", "=", id)]),
    )?;
    if found.is_empty() {
        return Err(ParamError::NoSuchRecord.into());
    }
    Ok(env.get_record(id.into()))
}
