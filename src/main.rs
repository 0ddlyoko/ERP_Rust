use rocket::{get, launch, routes};
use code_gen::{model, test_main};
use http::HttpStatus;
use my_macro::{log_entry_and_exit, log_entry_test, lol};
use std::fmt::{Display, Formatter};
// use my_macro::log_entry_and_exit;

// #[tokio::main]
// #[test_here]
// struct Test<'a> {
//     lol: &'a str,
// }

// #[launch]
// fn rocket() -> _ {
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

#[model("res.users")]
pub struct UserInit<'a> {
    name: &'a str,
}

#[model("player.age")]
pub struct PlayerAge {
    age: u32,
}

#[test_main]
fn test() {
    let a = HttpStatus::Continue;
    // my_function();
    // my_function();
    println!("{}", a);
    println!("{}", HttpStatus { code: 100 });
    println!("{}", HttpStatus { code: 504 });
    println!("{}", HttpStatus { code: 200 });
    println!("{}", HttpStatus { code: 201 });

    // let b = UserInit {
    //     name: "test"
    // };
    // println!("{}", b);
    // println!("{}", b.the_name());
    // test();
}

//
// #[macro_use]
// extern crate my_macro;

// use rocket::get;

// #[log_entry_and_exit("hello")]
// fn this_will_be_destroyed() -> u32 {
//     42
// }
//
// macro_rules! create_function {
//     ($func_name:ident) => {
//         fn $func_name() {
//             println!("You called {:?}()", stringify!($func_name));
//         }
//     }
// }
//
// macro_rules! bar {
//     (3) => {}
// }
//
// macro_rules! foo {
//     ($l:tt) => {
//         bar!($l);
//     }
// }

// create_function!(foo);
// create_function!(bar);

// fn main() {
//     foo!(3);
//     println!("{}", this_will_be_destroyed());
// }

//
// impl<'a> Test<'a> {
//     fn new(lol: &'a str) -> Self {
//         Test { lol }
//     }
//
//     fn show(&self) {
//         println!("{}", self.lol);
//     }
// }

// #[proc_macro_attribute]
// fn test_here() {
//     println!("haha")
// }

// fn hello(name: &str, age: u8) -> String {
//     return "Hello %s" % (name)
// }
