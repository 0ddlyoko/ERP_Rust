use erp::environment::Environment;
use erp::field::{EnumType, FieldDescriptor, FieldType};
use erp::model::{BaseModel, MapOfFields, Model, ModelDescriptor, SimplifiedModel};
use std::error::Error;

pub struct BaseSaleOrder;

impl BaseModel for BaseSaleOrder {
    fn get_model_name() -> &'static str {
        "sale_order"
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SaleOrderState {
    Draft,
    Sent,
    Paid,
    Cancelled,
}

impl EnumType for SaleOrderState {
    fn to_string(&self) -> String {
        match self {
            SaleOrderState::Draft => String::from("draft"),
            SaleOrderState::Sent => String::from("sent"),
            SaleOrderState::Paid => String::from("paid"),
            SaleOrderState::Cancelled => String::from("cancelled"),
        }
    }

    fn from_string(t: String) -> Self {
        match t.as_ref() {
            "draft" => SaleOrderState::Draft,
            "sent" => SaleOrderState::Sent,
            "paid" => SaleOrderState::Paid,
            "cancelled" => SaleOrderState::Cancelled,
            _ => SaleOrderState::Cancelled,
        }
    }
}

pub struct SaleOrder {
    pub id: u32,
    pub name: String,
    pub state: SaleOrderState,
    pub price: i64,
    pub amount: i64,
    pub total_price: i64,
}

impl SaleOrder {
    pub fn get_id(&self) -> u32 {
        self.id
    }

    pub fn get_name(&self) -> &String {
        &self.name
    }

    pub fn get_state(&self) -> &SaleOrderState {
        &self.state
    }

    pub fn get_price(&self) -> i64 {
        self.price
    }

    pub fn get_amount(&self) -> i64 {
        self.amount
    }

    pub fn get_total_price(&self) -> i64 {
        self.total_price
    }

    pub fn compute_total_price(
        &mut self,
        _environment: &Environment,
    ) -> Result<(), Box<dyn Error>> {
        self.total_price = self.price * self.amount;
        Ok(())
    }
}

impl Model for SaleOrder {
    type BaseModel = BaseSaleOrder;
}

impl SimplifiedModel for SaleOrder {
    fn get_model_descriptor() -> ModelDescriptor {
        ModelDescriptor {
            name: Self::get_model_name().to_string(),
            description: Some("A Sale Order!".to_string()),
            fields: vec![
                FieldDescriptor {
                    name: "name".to_string(),
                    default_value: Some(FieldType::String("0ddlyoko".to_string())),
                    description: Some("Name of the SO".to_string()),
                    required: true,
                    ..FieldDescriptor::default()
                },
                FieldDescriptor {
                    name: "state".to_string(),
                    default_value: Some(FieldType::Enum(SaleOrderState::Draft.to_string())),
                    description: Some("Current state of this SO".to_string()),
                    required: true,
                    ..FieldDescriptor::default()
                },
                FieldDescriptor {
                    name: "price".to_string(),
                    default_value: Some(FieldType::Integer(42)),
                    description: Some("Unit price".to_string()),
                    ..FieldDescriptor::default()
                },
                FieldDescriptor {
                    name: "amount".to_string(),
                    default_value: Some(FieldType::Integer(10)),
                    description: Some("Quantity".to_string()),
                    ..FieldDescriptor::default()
                },
                FieldDescriptor {
                    name: "total_price".to_string(),
                    default_value: Some(FieldType::Integer(0)),
                    description: Some("Total price of the SO".to_string()),
                    compute: Some(true),
                    ..FieldDescriptor::default()
                },
            ],
        }
    }

    fn get_id(&self) -> u32 {
        self.id
    }

    fn get_data(&self) -> MapOfFields {
        let mut result = MapOfFields::default();
        result.insert("name", self.get_name());
        result.insert("state", self.get_state());
        result.insert("price", self.get_price());
        result.insert("amount", self.get_amount());
        result.insert("total_price", self.get_total_price());
        result
    }

    fn create_model(id: u32, data: MapOfFields) -> Self {
        Self {
            id,
            name: data.get("name"),
            state: data.get("state"),
            price: data.get("price"),
            amount: data.get("amount"),
            total_price: data.get("total_price"),
        }
    }

    fn call_compute_method(
        &mut self,
        field_name: &str,
        env: &mut Environment,
    ) -> Result<(), Box<dyn Error>> {
        if field_name == "total_price" {
            return self.compute_total_price(env);
        }
        Ok(())
    }
}
