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
