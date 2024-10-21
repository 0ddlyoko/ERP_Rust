use std::fmt::{Display, Formatter};

pub enum Method {
    Get,
    Put,
    Post,
    Delete,
    Options,
    Patch,
    Unknown,
}

macro_rules! make_method {
    ($($fnc_name: ident, $name: expr),+) => {

        pub fn name(&self) -> &'static str {
            match self {
                $(Method::$fnc_name => $name,)+
            }
        }

        pub fn from_string(string: &str) -> Method {
            match string {
                $($name => Method::$fnc_name,)+
                _ => Method::Unknown,
            }
        }
        // $(
            // #[allow(non_upper_case_globals)]
            // pub const $fnc_name: Method = Method { code: $code };
        // )+

        // pub fn reason(&self) -> Option<&'static str> {
        //     match self.code {
        //         $($code => Some($name),)+
        //         _ => None
        //     }
        // }
    }
}

impl Method {
    make_method! {
        Get, "Get",
        Put, "Put",
        Post, "Post",
        Delete, "Delete",
        Options, "Options",
        Patch, "Patch",
        Unknown, "Unknown"
    }

    // pub fn from_string(string: &str) -> Method {
    //     match string {
    //         "Delete" => Method::Delete,
    //         _ => Method::Get,
    //     }
    // }
}

impl Display for Method {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Method {}", self.name())
    }
}
