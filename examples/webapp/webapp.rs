//! Build the Quickstart's web app with the builder and print it as the interactive SVG.

use svugy::{InteractiveSvg, RenderSvg};
use svugy_model::builder::ModelBuilder;
use svugy_model::model::{Edge, Element, Shape, Traveler};
use svugy_model::workflow::{Step, StepId, Workflow, WorkflowId};

const SERVICE_FILL: &str = "#3478f6";
const STORE_FILL: &str = "#d5dcea";

fn main() {
    let mut builder = ModelBuilder::new();

    let browser_element = Element::new("Browser").with_caption("the user's tab");
    let browser = builder.add_top_level("browser", browser_element);
    let cdn_element = Element::new("CDN")
        .with_caption("static files")
        .with_shape(Shape::Disc);
    let cdn = builder.add_top_level("cdn", cdn_element);

    let backend_element = Element::new("Backend")
        .with_caption("your servers")
        .with_description("Every browser request lands on [the API](#backend.api). From there:\n- the API answers the browser right away\n- work that can wait, like the receipt, goes through [the queue](#backend.queue) to [the worker](#backend.worker)");
    let backend = builder.add_top_level("backend", backend_element);
    let api_element = Element::new("API")
        .with_caption("HTTP endpoints")
        .with_style("fill", SERVICE_FILL);
    let api = builder.add_child(&backend, "api", api_element);
    let queue_element = Element::new("Queue")
        .with_caption("pending jobs")
        .with_shape(Shape::Slab);
    let queue = builder.add_child(&backend, "queue", queue_element);
    let worker_element = Element::new("Worker")
        .with_caption("background jobs")
        .with_style("fill", SERVICE_FILL);
    let worker = builder.add_child(&backend, "worker", worker_element);

    let data_element = Element::new("Data")
        .with_caption("what the app remembers")
        .with_description(
            "[The database](#data.db) keeps users and orders. [The cache](#data.cache) keeps the reads the API repeats most.",
        );
    let data = builder.add_top_level("data", data_element);
    let db_element = Element::new("Database")
        .with_caption("users · orders")
        .with_shape(Shape::Cylinder)
        .with_style("fill", STORE_FILL);
    let db = builder.add_child(&data, "db", db_element);
    let cache_element = Element::new("Cache")
        .with_caption("hot reads")
        .with_shape(Shape::Cylinder)
        .with_style("fill", STORE_FILL);
    let cache = builder.add_child(&data, "cache", cache_element);

    let mail_element = Element::new("Email provider")
        .with_caption("outside your servers")
        .with_style("fill", "transparent")
        .with_style("stroke-dash", "5");
    let mail = builder.add_top_level("mail", mail_element);

    let main_id = WorkflowId::new("main");
    let main_workflow = Workflow::new(main_id, "A web app takes an order")
        .with_subtitle("from the browser to the database and back")
        .with_description(
            "A user opens the app, signs in and buys. [The API](#backend.api) answers every request at once; [the receipt](#main/receipt) is sent later.",
        );
    let main = builder.add_workflow(main_workflow);

    let load_id = StepId::new("load");
    let load_step = Step::new(load_id, "The browser loads the page").with_description(
        "[The CDN](#cdn) serves the HTML, scripts and images; no request reaches your servers yet.",
    );
    let load = builder.add_step(&main, load_step);
    let loads_page = Edge::new().with_label("loads the page");
    builder.add_edge(&load, &browser, &cdn, loads_page);

    let sign_in_id = StepId::new("sign-in");
    let sign_in_step = Step::new(sign_in_id, "The user signs in").with_description(
        "[The API](#backend.api) checks the password and stores the session in [the cache](#data.cache), so later requests skip the database.",
    );
    let sign_in = builder.add_step(&main, sign_in_step);
    let signs_in = Edge::new()
        .with_label("signs in")
        .with_traveler(Traveler::Key);
    builder.add_edge(&sign_in, &browser, &api, signs_in);
    let stores_session = Edge::new().with_label("stores the session");
    builder.add_edge(&sign_in, &api, &cache, stores_session);

    let order_id = StepId::new("order");
    let order_step = Step::new(order_id, "The user places an order").with_description(
        "[The API](#backend.api) reads [the session](#backend.api->data.cache[1]) back and saves the order in [the database](#data.db).",
    );
    let order = builder.add_step(&main, order_step);
    let places_order = Edge::new()
        .with_label("places an order")
        .with_description("The request body:\n```ts\ntype PlaceOrder = {\n  items: { sku: string; quantity: number }[];\n  paymentToken: string;\n};\n```")
        .with_traveler(Traveler::Envelope);
    builder.add_edge(&order, &browser, &api, places_order);
    let reads_session = Edge::new().with_label("reads the session");
    builder.add_edge(&order, &api, &cache, reads_session);
    let saves_order = Edge::new()
        .with_label("saves the order")
        .with_traveler(Traveler::Document);
    builder.add_edge(&order, &api, &db, saves_order);

    let receipt_id = StepId::new("receipt");
    let receipt_step = Step::new(receipt_id, "The worker sends the receipt").with_description(
        "[The API](#backend.api) only queues the receipt; [the worker](#backend.worker) picks it up and hands it to [the email provider](#mail).",
    );
    let receipt = builder.add_step(&main, receipt_step);
    let schedules_receipt = Edge::new()
        .with_label("schedules a receipt")
        .with_description("The API answers [the browser](#browser) *before* the receipt is sent.")
        .with_traveler(Traveler::Parcel);
    builder.add_edge(&receipt, &api, &queue, schedules_receipt);
    let hands_over = Edge::new().with_label("hands over");
    builder.add_edge(&receipt, &queue, &worker, hands_over);
    let sends_receipt = Edge::new()
        .with_label("sends the receipt")
        .with_description("The email provider delivers the receipt to the user's inbox.")
        .with_traveler(Traveler::Envelope);
    builder.add_edge(&receipt, &worker, &mail, sends_receipt);

    let admission = builder.build();
    let Some(model) = admission.model else {
        let report_json = admission.report.to_json();
        eprintln!("{report_json}");
        std::process::exit(1);
    };
    let svg = InteractiveSvg::render(model.showing_first_workflow());
    print!("{svg}");
}
