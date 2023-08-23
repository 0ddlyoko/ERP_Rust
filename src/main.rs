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
// use test_http::Method::Post;


use test_lib::*;


model! {
    #[derive(Debug)]
    #[odd(table_name = "sale_order")]
    pub struct SaleOrder {
        pub title: Field<String>,
        // pub title: &Field<String>,
        pub body: Field<String>,
        #[odd(default = "true")]
        pub published: Field<bool>,
        #[odd(default = "42")]
        pub lol: Field<i32>,
    }
}

model! {
    #[derive(Debug)]
    #[odd(table_name = "sale_order_line")]
    pub struct SaleOrderLine {
        pub name: Field<String>,
        pub sale_order: Field<Many2one<SaleOrder>>,
    }
}

impl<'env> Display for SaleOrder<'env> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "id: {}, title: {:?}, body: {:?}, published: {:?}", self.id, self.title, self.body, self.published)
    }
}

fn main() {
    let mut global_env = GlobalEnvironment::new();
    let model_manager = global_env.models_mut();
    model_manager.register::<SaleOrder>("module_a");

    let models = model_manager.models();
    for (name, model_descriptor) in models {
        println!("{}, {:?}", name, model_descriptor);
    }

    let mut env = std::rc::Rc::new(std::cell::RefCell::new(global_env.new_env()));
    let mut sale_order: SaleOrder = SaleOrder::new::<SaleOrder>(std::rc::Rc::downgrade(&env));
    let id = sale_order.id;
    sale_order.title.set("SALUT, JE SUIS AUDD :D".to_string());
    sale_order.published.set(false);
    sale_order.save();
    sale_order.save();
}
