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
    #[odd(table_name = "res_partner")]
    pub struct Post {
        pub title: Field<String>,
        // pub title: &Field<String>,
        pub body: Field<String>,
        #[odd(default = "true")]
        pub published: Field<bool>,
        #[odd(default = "42")]
        pub lol: Field<i32>,
    }
}

impl<'env> Post<'env> {
    fn test(&self) -> HashMap<String, FieldType> {
        let mut map = HashMap::new();
        map.insert("title".to_string(), FieldType::String(Field::new(self.title.value().clone())));
        map
    }
    // fn _from_map(id: u32, mut map: HashMap<String, FieldType>, env: std::rc::Weak<std::cell::RefCell<Environment<'env>>>) -> Self {
    //     Self {
    //         id: id,
    //         title: FieldType::transform_to_string(map.remove("title").unwrap()),
    //         body: FieldType::transform_to_string(map.remove("body").unwrap()),
    //         published: FieldType::transform_to_boolean(map.remove("published").unwrap()),
    //         lol: FieldType::transform_to_integer(map.remove("lol").unwrap()),
    //         _env: env,
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
    pub struct Post2 {
        pub title: Field<String>,
        pub author: Field<String>,
        #[odd(required)]
        pub published: Field<bool>,
        // #[odd(default="45")]
        pub lol: Field<i32>,
    }
}

// impl SaleOrder {
//     pub fn get_partner(&self) {
//         self._env.
//     }
// }

impl<'env> Display for Post<'env> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "id: {}, title: {:?}, body: {:?}, published: {:?}", self.id, self.title, self.body, self.published)
    }
}

// impl ModelEnvironment for Post {
//     my_env : &Environment,
//     fn env(&self) -> &Environment {
//         return self.my_env;
//     }
// }
pub struct Test<'env> {
    pub id: i32,
    pub title: String,
    pub author: String,
    pub published: bool,
    pub _env: &'env mut Environment<'env>,
}

struct Parent<'a> {
    name: String,
    children: Vec<std::rc::Rc<Child<'a>>>,
}

struct Child<'a> {
    name: String,
    parent: std::rc::Weak<std::cell::RefCell<Parent<'a>>>,
}

fn main() {
    // // TODO USE Rc & Weak like here for Environment in models
    // let mut parent = std::rc::Rc::new(std::cell::RefCell::new(Parent {
    //     name: "Parent".to_string(),
    //     children: Vec::new(),
    // }));
    // let child1 = Child {
    //     name: "Child 1".to_string(),
    //     parent: std::rc::Rc::downgrade(&parent),
    // };
    // let child2 = Child {
    //     name: "Child 2".to_string(),
    //     parent: std::rc::Rc::downgrade(&parent),
    // };
    // let child3 = Child {
    //     name: "Child 3".to_string(),
    //     parent: child2.parent.clone(),
    // };
    //
    // match child1.parent.upgrade() {
    //     Some(a) => {
    //         println!("{}", a.borrow().name);
    //     }
    //     None => {
    //         println!("EMPTY")
    //     }
    // }
    //
    // // parent = std::rc::Rc::new(std::cell::RefCell::new(Parent {
    // //     name: "Parent 2".to_string(),
    // //     children: Vec::new(),
    // // }));
    //
    // match child1.parent.upgrade() {
    //     Some(rc) => {
    //         let mut borrow = rc.borrow_mut();
    //         println!("{}", borrow.name);
    //         borrow.name = "new name".to_string();
    //     }
    //     None => {
    //         println!("EMPTY")
    //     }
    // }
    //
    // match child1.parent.upgrade() {
    //     Some(rc) => {
    //         let borrow = rc.borrow();
    //         println!("{}", borrow.name);
    //     }
    //     None => {
    //         println!("EMPTY")
    //     }
    // }

    let mut global_env = GlobalEnvironment::new();
    let model_manager = global_env.models_mut();
    model_manager.register::<Post>("module_a");
    // model_manager.register::<Post2>("module_b");

    let models = model_manager.models();
    for (name, model_descriptor) in models {
        println!("{}, {:?}", name, model_descriptor);
    }

    let mut env = std::rc::Rc::new(std::cell::RefCell::new(global_env.new_env()));
    let mut my_post: Post = Post::new::<Post>(std::rc::Rc::downgrade(&env));
    let id = my_post.id;
    println!("{}, {:?}", my_post.id, my_post.published.value());
    my_post.title.set("SALUT, JE SUIS AUDD :D".to_string());
    my_post.published.set(false);
    println!("{}, {:?}", my_post.id, my_post.published.value());
    my_post.save();

    let mut my_post_2: Post = Post::load(id, std::rc::Rc::downgrade(&env));
    println!("{}, {:?}, {:?}", my_post.id, my_post.title.value(), my_post.published.value());
    println!("{}, {:?}, {:?}", my_post_2.id, my_post_2.title.value(), my_post_2.published.value());


    // let a = std::rc::Weak::new();
    // let id = env.counter;
    // env.counter += 1;
    // let cached_record = env.cache_mut().new_cached_record("res_partner", id);
    // let a = Post::_from_map(id, cached_record.fields_mut(), env);

    // let model = env.new_model::<Post>();

    // Post::_from_map(42, &mut HashMap::new(), &mut env);

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
