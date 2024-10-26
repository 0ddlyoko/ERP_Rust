// use std::collections::HashMap;
// use std::fmt::{Display, Formatter};
// use std::string::ToString;

// use old_code_gen::*;
// use old_test_lib::*;


// model! {
//     #[derive(Debug)]
//     #[odd(table_name = "sale_order")]
//     pub struct SaleOrder {
//         pub title: Field<String>,
//         // pub title: &Field<String>,
//         pub body: Field<String>,
//         #[odd(default = "true")]
//         pub published: Field<bool>,
//         #[odd(default = "42")]
//         pub lol: Field<i32>,
//     }
// }
//
// // model! {
// //     #[derive(Debug)]
// //     #[odd(table_name = "sale_order_line")]
// //     pub struct SaleOrderLine {
// //         pub name: Field<String>,
// //         // pub sale_order: Field<Many2one<SaleOrder>>,
// //     }
// // }
//
// // model! {
// //     #[derive(Debug)]
// //     #[odd(table_name = "sale_order_copy")]
// //     pub struct SaleOrderCopy {
// //         // Existing fields
// //         pub title: Field<String>,
// //         // New fields
// //         #[odd(default = "0ddlyoko")]
// //         pub author: Field<String>,
// //     }
// // }
// //
// // impl<'env> SaleOrderCopy<'env> {
// //     fn print_author_and_title(&self) {
// //         println!("author = {}, title = {}", self.author, self.title);
// //     }
// // }
//
// impl Display for SaleOrder {
//     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
//         write!(f, "id: {}, title: {:?}, body: {:?}, published: {:?}", self.id, self.title, self.body, self.published)
//     }
// }

// fn main() {
    // let a = &"test".to_string();
    // if a.eq("test") {
    //
    // }
    // let mut global_env = GlobalEnvironment::new();
    // let model_manager = global_env.models_mut();
    // model_manager.register::<SaleOrder>("module_a");
    // // model_manager.register::<SaleOrderCopy>("module_b");
    //
    // let mut env = global_env.new_env();
    // let mut sale_order: SaleOrder = SaleOrder::new(&mut env);
    // sale_order.title.set("SALUT, JE SUIS AUDD :D".to_string());
    // sale_order.published.set(false);
    //
    //
    // let name = SaleOrder::_name();
    // // sale_order.save();
    //
    // // let sale_order_copy: SaleOrderCopy = sale_order.convert_to();
    // // // println!("{:?}", sale_order_copy);
    // // sale_order_copy.print_author_and_title();
    //
    // println!("{:?} {:?}", sale_order.title, sale_order.published);
    //
    // sale_order.update(HashMap::from([
    //     ("title", Some("New title")),
    //     ("published", Some("true")),
    // ]));
    // // sale_order_copy.print_author_and_title();
    // println!("{:?} {:?}", sale_order.title, sale_order.published);
// }

use std::collections::HashMap;

struct SaleOrder {
    id: u32,
    name: String,
}

impl SaleOrder {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn get_name(&self) -> String {
        self.name.clone()
    }
}

impl core::model::Model for SaleOrder {
    fn get_model_name() -> &'static str {
        "sale_order"
    }

    fn get_model_descriptor() -> core::model::ModelDescriptor {
        core::model::ModelDescriptor {
            name: "sale_order".to_string(),
            description: "A Sale Order!".to_string(),
            fields: vec![
                core::field::FieldDescriptor {
                    name: "name".to_string(),
                    field_type: core::field::FieldType::String(String::new()),
                    description: Some("Name of the SO".to_string()),
                    required: Some(true),
                    ..core::field::FieldDescriptor::default()
                }
            ],
        }
    }

    fn get_id(&self) -> u32 {
        self.id
    }

    fn get_data(&self) -> core::model::MapOfFields {
        let mut map = HashMap::new();
        map.insert("name", Some(core::field::FieldType::String(self.name.clone())));
        map
    }

    fn create_model(id: u32, data: core::model::MapOfFields) -> Self {
        let name = data["name"].clone().unwrap().string();
        Self {
            id,
            name,
        }
    }
}

fn main() {
    let mut model_manager = core::model::ModelManager::new();
    model_manager.register_model::<SaleOrder>();
    
    let mut env: core::environment::Environment = model_manager.new_environment();
    
    let record = env.get_record_from_name("test", 1);
    println!("{}", record.unwrap().is_some());
    
    // let mut a: core::model::MapOfFields = HashMap::new();
    // a.insert("name".to_string(), Some(core::field::FieldType::String("0ddlyoko".to_string())));
    // let z = model_manager.create_instance::<SaleOrder>(1, a);
    // match z {
    //     None => {
    //         println!("No model found");
    //     }
    //     Some(some) => {
    //         println!("Model found");
    //         println!("{:?}", some.get_id());
    //         println!("{:?}", some.get_name());
    //     }
    // }
}
