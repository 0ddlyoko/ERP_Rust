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
use diesel::sql_types::Integer;
use rocket::figment::providers::Env;
use rocket::form::Context;
use test_http::Method::Post;


use test_lib::*;


model! {
    #[derive(Debug)]
    #[odd(table_name = "res_partner")]
    pub struct Post<'field> {
        pub title: &'field Field<String>,
        // pub title: &'field Option<String>,
        pub body: &'field Field<String>,
        #[odd(default = "true")]
        pub published: &'field Field<bool>,
        #[odd(default = "42")]
        pub lol: &'field Field<i32>,
    }
}

impl Post<'_, '_> {
    pub fn get_id(&self) -> u32 {
        self.id
    }

    // pub fn get_title(&self) -> &Option<String> {
    //     self.title
    // }
    //
    // pub fn get_body(&self) -> &Option<String> {
    //     self.body
    // }
    //
    // pub fn get_published(&self) -> &Option<bool> {
    //     self.published
    // }
    //
    // pub fn set_title(&mut self, title: Option<String>) {
    //     // self.title = title;
    //     // TODO Change in cache
    // }

    // pub fn from<'env, 'field>(env: &Environment, map: HashMap<String, &Field>) -> Post<'env, 'field> {
    //     let id = map["id"];
    //     let title = map["title"];
    //     let body = map["body"];
    //     let published = map["published"];
    //
    //     Post {
    //         _env: env,
    //         id: id,
    //         title: title,
    //         body: body,
    //         published: published,
    //     }
    // }
}

// impl<'env, 'a> Post<'env, 'a> {

//     fn test(&mut self) {
//         let old_context = self._env.with_new_context("skip_backorder", "true");
//         // Call SO
//
//         // Restore old context
//         self._env.restore_context(old_context);
//     }
// }

model! {
    #[derive(Debug)]
    #[odd(table_name = "res_partner")]
    pub struct Post2<'field> {
        pub title: &'field Field<String>,
        pub author: &'field Field<String>,
        #[odd(required)]
        pub published: &'field Field<bool>,
        // #[odd(default="45")]
        pub lol: &'field Field<i32>,
    }
}

// impl SaleOrder {
//     pub fn get_partner(&self) {
//         self._env.
//     }
// }

impl<'env, 'field> Display for Post<'env, 'field> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "id: {}, title: {:?}, body: {:?}, published: {:?}", self.id, self.title, self.body, self.published)
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
pub struct Test<'env> {
    pub id: i32,
    pub title: String,
    pub author: String,
    pub published: bool,
    pub _env: &'env mut Environment<'env>,
}

fn main() {
    let mut global_env = GlobalEnvironment::new();
    let model_manager = global_env.models_mut();
    model_manager.register::<Post>("module_a");
    model_manager.register::<Post2>("module_b");

    let models = model_manager.models();
    for (name, model_descriptor) in models {
        println!("{}, {:?}", name, model_descriptor);
    }

    let mut env = global_env.new_env();

    let model = env.new_model::<Post>();

    // let post = Post {
    //     id: 1,
    //     title: &Option::from("Hello".to_string()),
    //     body: &Option::from("World".to_string()),
    //     published: &Option::from(true),
    //     _env: &mut env,
    // };

    // let a = post._env.with_context("test", "true");

    // let post2: Post2 = post.into();
}
