use std::collections::HashMap;
use std::fmt::{Display, Formatter, Pointer};
use std::string::ToString;

use code_gen::*;
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
        // pub sale_order: Field<Many2one<SaleOrder>>,
    }
}

model! {
    #[derive(Debug)]
    #[odd(table_name = "sale_order")]
    pub struct SaleOrderCopy {
        // Existing fields
        pub title: Field<String>,
        // New fields
        #[odd(default = "0ddlyoko")]
        pub author: Field<String>,
    }
}

impl<'env> SaleOrderCopy<'env> {
    fn print_author_and_title(&self) {
        println!("author = {}, title = {}", self.author, self.title);
    }
}

impl<'env> Display for SaleOrder<'env> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "id: {}, title: {:?}, body: {:?}, published: {:?}", self.id, self.title, self.body, self.published)
    }
}

fn main() {
    let a = &"test".to_string();
    if a.eq("test") {

    }
    let mut global_env = GlobalEnvironment::new();
    let model_manager = global_env.models_mut();
    model_manager.register::<SaleOrder>("module_a");
    model_manager.register::<SaleOrderCopy>("module_b");

    let env = std::rc::Rc::new(std::cell::RefCell::new(global_env.new_env()));
    let mut sale_order: SaleOrder = SaleOrder::new::<SaleOrder>(std::rc::Rc::downgrade(&env));
    sale_order.title.set("SALUT, JE SUIS AUDD :D".to_string());
    sale_order.published.set(false);
    // sale_order.save();

    let mut sale_order_copy: SaleOrderCopy = sale_order.convert_to();
    // println!("{:?}", sale_order_copy);
    sale_order_copy.print_author_and_title();
}
