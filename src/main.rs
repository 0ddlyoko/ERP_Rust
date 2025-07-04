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

use erp::app::Application;
use erp::config::Config;

fn main() {
    let config = Config::try_default().unwrap_or_else(|err| panic!("Error while deserializing config: {:?}", err));
    let mut app = Application::new(config);

    app.load().unwrap_or_else(|err| panic!("Error: {}", err));

    let model = app.model_manager.get_model("sale_order_test");
    println!("Models: {}", model.name);

    app.unload();
}
