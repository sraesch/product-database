use std::{env::temp_dir, str::FromStr};

use chrono::Local;
use dockertest::{
    DockerTest, Image, LogAction, LogOptions, LogPolicy, LogSource, TestBodySpecification,
};
use log::info;
use product_db::{
    DataBackend, PostgresBackend, PostgresConfig, ProductInfo, ProductRequest, Secret,
};

/// Initialize the logger for the tests.
fn init_logger() {
    match env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Trace)
        .try_init()
    {
        Ok(_) => (),
        Err(_) => println!("Logger already initialized"),
    }
}

/// Loads the product data from the test_data/products.json file.
fn load_products() -> Vec<ProductInfo> {
    let product_data = include_str!("../../test_data/products.json");
    serde_json::from_str(product_data).unwrap()
}

/// Runs the backend tests with the given backend.
///
/// # Arguments
/// - `backend` - The backend to run the tests with.
async fn backend_tests<B: DataBackend>(backend: B) {
    // report a missing product twice
    let id1 = backend
        .report_missing_product("missing-product".to_string(), Local::now())
        .await
        .unwrap();
    let id2 = backend
        .report_missing_product("missing-product".to_string(), Local::now())
        .await
        .unwrap();
    assert_ne!(id1, id2, "The ids should be different");
    info!("Reported missing products with ids: {} and {}", id1, id2);

    // delete the first reported missing product
    backend.delete_reported_missing_product(id1).await.unwrap();

    // try again to delete the first reported missing product
    backend.delete_reported_missing_product(id1).await.unwrap();

    // load the products from the test_data/products.json file
    let products = load_products();

    // request the products in the list
    for product_info in products.iter() {
        let product_request = ProductRequest {
            product_info: product_info.clone(),
            date: Local::now(),
            product_photo: None,
        };

        let id = backend.request_new_product(&product_request).await.unwrap();
        info!("Requested product with id: {}", id);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_postgres_backend() {
    init_logger();

    // Define our test instance
    let mut test = DockerTest::new();

    let image: Image = Image::with_repository("postgres")
        .pull_policy(dockertest::PullPolicy::IfNotPresent)
        .source(dockertest::Source::DockerHub)
        .tag("16");

    // define the postgres container
    let mut postgres = TestBodySpecification::with_image(image).set_publish_all_ports(true);

    // set the environment variables for the postgres container
    postgres
        .modify_env("POSTGRES_USER", "postgres")
        .modify_env("POSTGRES_PASSWORD", "password");

    let mut postgres = postgres.set_log_options(Some(LogOptions {
        action: LogAction::ForwardToStdOut,
        policy: LogPolicy::Always,
        source: LogSource::Both,
    }));

    // create a temporary file to store the database schema
    let schema = include_str!("../../database/init.sql");
    let mut init_file = temp_dir();
    init_file.push("init.sql");
    std::fs::write(&init_file, schema).unwrap(); // write the schema to a file

    // bind the schema file to the postgres container
    postgres.modify_bind_mount(
        init_file.to_string_lossy(),
        "/docker-entrypoint-initdb.d/init.sql",
    );

    // run the postgres container
    test.provide_container(postgres);

    test.run_async(|ops| async move {
        let container = ops.handle("postgres");

        // wait about 5 seconds for postgres to start
        info!("Waiting for postgres to start...");
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        info!("Waiting for postgres to start...DONE");

        let (ip, port) = container.host_port(5432).unwrap();
        info!("postgres running at {}:{}", ip, port);

        let options = PostgresConfig {
            host: "localhost".to_string(),
            port: *port as u16,
            dbname: "postgres".to_string(),
            user: "postgres".to_string(),
            password: Secret::from_str("password").unwrap(),
        };

        info!("Creating PostgresBackend instance...");
        let postgres_backend = PostgresBackend::new(options).await.unwrap();
        info!("Creating PostgresBackend instance...DONE");

        info!("Running backend tests...");
        backend_tests(postgres_backend).await;
        info!("Running backend tests...SUCCESS");
    })
    .await;
}
