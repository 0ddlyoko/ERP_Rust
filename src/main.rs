use std::collections::HashMap;
use rocket::{get, launch, routes};
use test_http::HttpStatus;
use my_macro::{log_entry_and_exit, log_entry_test, lol};
use std::fmt::{Display, Formatter, Pointer};
use std::ptr::hash;
use std::string::ToString;
use diesel::IntoSql;
// use my_macro::log_entry_and_exit;

// #[tokio::main]
// #[test_here]
// struct Test<'a> {
//     lol: &'a str,
// }

// #[launch]
// fn rocket() -> _ {
//     let a = routes![hello];
//     println!("{:?}", a);
//     rocket::build().mount("/hello", routes![hello])
// }
//
// #[get("/<name>/<age>")]
// fn hello(name: &str, age: u8) -> String {
//     format!("Hello, {} year old named {}!", age, name)
// }

// #[log_entry_and_exit]
// fn my_function() {
//     println!("LOL");
// }

// #[lol]
// fn mdrr() {
//     println!("TEST");
// }

// use diesel::prelude::*;
//
// #[derive(Queryable, Selectable)]
// #[diesel(table_name = crate::schema::posts)]
// #[diesel(check_for_backend(diesel::pg::Pg))]
// #[loll]
// pub struct Post {
//     pub id: i32,
//     pub title: String,
//     pub body: String,
//     pub published: bool,
// }




// pub struct Player {
//     pub id: i32,
//     pub name: String,
// }
//
// impl Player {
//     pub const model_data: &'static str = "4";
// }
//
// pub struct PlayerWithDate {
//     pub id: i32,
//     pub date: String,
// }







use code_gen::*;


use test_lib::*;

// #[macro_export]
macro_rules! model {
    (
        $(#[$derive_2:meta])*
        $pub:vis struct $name:ident $(<$($a:tt),*>)? {
            $($fields:tt)*
        }
    ) => {
        #[derive(Model)]
        $(#[$derive_2])*
        $pub struct $name<'env$(, $($a),*)?> {
            env: &'env Environment<'env>,
            $($fields)*
        }
    }
}

model! {
    #[derive(Debug)]
    #[odd(table_name = "post")]
    pub struct Post<'a> {
        #[odd(required)]
        pub id: i32,
        pub title: &'a String,
        pub body: String,
        pub published: bool,
    }
}

model! {
    #[derive(Debug)]
    #[odd(table_name = "post")]
    pub struct Post2 {
        pub id: i32,
        pub title: String,
        pub author: String,
        #[odd(required)]
        pub published: bool,
    }
}

impl<'env, 'a> Display for Post<'env, 'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "id: {}, title: {}, body: {}, published: {}", self.id, self.title, self.body, self.published)
    }
}

// impl ModelEnvironment for Post {
//     my_env : &Environment,
//     fn env(&self) -> &Environment {
//         return self.my_env;
//     }
//
//     fn restore_env(&self, env: Environment) {
//         self.my_env = env;
//     }
// }

fn main() {
    let mut model_manager = ModelManager::new();
    model_manager.register::<Post>("module_a");
    model_manager.register::<Post2>("module_b");

    let models = model_manager.models();
    for (name, model_descriptor) in models {
        println!("{}, {:?}", name, model_descriptor);
    }
}
