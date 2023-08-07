use rocket::{get, launch, routes};
use test_http::HttpStatus;
use my_macro::{log_entry_and_exit, log_entry_test, lol};
use std::fmt::{Display, Formatter, Pointer};
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




pub struct Player {
    pub id: i32,
    pub name: String,
}

impl Player {
    pub const model_data: &'static str = "4";
}

pub struct PlayerWithDate {
    pub id: i32,
    pub date: String,
}







use code_gen::*;
use test_lib::*;

// const a: &InternalModel;


#[derive(Model, Debug)]
#[odd(table_name = "post")]
pub struct Post {
    pub id: i32,
    #[odd(required)]
    pub title: String,
    pub body: String,
    pub published: bool,
}

#[derive(Model, Debug)]
// #[odd(is_copy = "True")]
#[odd(table_name = "post_2")]
pub struct Post2 {
    pub title: String,
    pub id: i32,
    pub body: String,
    pub published: bool,
}

#[derive(Model)]
#[odd(table_name = "test_enum")]
enum Test {

}

impl Display for Post {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "id: {}, title: {}, body: {}, published: {}", self.id, self.title, self.body, self.published)
    }
}

fn main() {
    let a = Post {
        id: 10,
        title: "Salut".to_string(),
        body: "Bonjour :D".to_string(),
        published: true,
    };

    println!("{}", a);
    println!("Post table_name = {}", a._name());
    println!("Post _get_fields = {:?}", a._get_fields());
    println!("Post _get_fields_required = {:?}", a._get_fields_required());

    let a = Post::_get_model;
    let b = Post2::_get_model;

    let models: Vec<fn() -> &'static str> = vec![a, b];

    vec![&Post, &Post2];
}

fn register<E>(models: Vec<fn() -> &'static str>) {
    for model in models {
        println!("{:?}", model())
    }
}
