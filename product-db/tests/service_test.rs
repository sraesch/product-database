use std::{env::temp_dir, str::FromStr, sync::Arc};

use chrono::{DateTime, Utc};
use dockertest::{
    DockerTest, Image, LogAction, LogOptions, LogPolicy, LogSource, TestBodySpecification,
};
use log::info;
use product_db::{
    DBId, DataBackend, Nutrients, Options, PostgresBackend, PostgresConfig, ProductDescription,
    ProductRequest, Secret, Service, Weight,
};

/// Truncates the given datetime to seconds.
/// This is being done for comparison reasons.
///
/// # Arguments
/// - `d` - The datetime to truncate.
fn truncate_datetime(d: DateTime<Utc>) -> DateTime<Utc> {
    let secs = d.timestamp();

    DateTime::from_timestamp(secs, 0).unwrap()
}

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
fn load_products() -> Vec<ProductDescription> {
    let product_data = include_str!("../../test_data/products.json");
    serde_json::from_str(product_data).unwrap()
}

/// Slightly lossy comparison of two weights.
///
/// # Arguments
/// - `lhs` - The left hand side of the comparison.
/// - `rhs` - The right hand side of the comparison.
fn compare_lossy_weights(lhs: Weight, rhs: Weight) -> bool {
    let eps = 1e-5;
    (lhs.value - rhs.value).abs() < eps
}

/// Slightly lossy comparison of two optional weights.
///
/// # Arguments
/// - `lhs` - The left hand side of the comparison.
/// - `rhs` - The right hand side of the comparison.
fn compare_lossy_weights_opt(lhs: Option<Weight>, rhs: Option<Weight>) -> bool {
    match (lhs, rhs) {
        (Some(lhs), Some(rhs)) => compare_lossy_weights(lhs, rhs),
        (None, None) => true,
        _ => false,
    }
}

/// Slightly lossy comparison of two nutrients.
///
/// # Arguments
/// - `lhs` - The left hand side of the comparison.
/// - `rhs` - The right hand side of the comparison.
fn check_compare_nutrients(lhs: &Nutrients, rhs: &Nutrients) {
    let eps = 1e-5;

    assert!((lhs.kcal - rhs.kcal) <= eps, "kcal are different");
    assert!(
        compare_lossy_weights_opt(lhs.carbohydrates, rhs.carbohydrates),
        "carbohydrates are different"
    );
    assert!(
        compare_lossy_weights_opt(lhs.fat, rhs.fat),
        "fat are different"
    );
    assert!(
        compare_lossy_weights_opt(lhs.protein, rhs.protein),
        "protein are different"
    );

    assert!(
        compare_lossy_weights_opt(lhs.sugar, rhs.sugar),
        "sugar are different"
    );
    assert!(
        compare_lossy_weights_opt(lhs.salt, rhs.salt),
        "salt are different"
    );

    assert!(
        compare_lossy_weights_opt(lhs.vitamin_a, rhs.vitamin_a),
        "vitamin_a are different"
    );
    assert!(
        compare_lossy_weights_opt(lhs.vitamin_c, rhs.vitamin_c),
        "vitamin_c are different"
    );
    assert!(
        compare_lossy_weights_opt(lhs.vitamin_d, rhs.vitamin_d),
        "vitamin_d are different"
    );

    assert!(
        compare_lossy_weights_opt(lhs.iron, rhs.iron),
        "iron are different"
    );
    assert!(
        compare_lossy_weights_opt(lhs.calcium, rhs.calcium),
        "calcium are different"
    );
    assert!(
        compare_lossy_weights_opt(lhs.magnesium, rhs.magnesium),
        "magnesium are different"
    );
    assert!(
        compare_lossy_weights_opt(lhs.sodium, rhs.sodium),
        "sodium are different"
    );
    assert!(
        compare_lossy_weights_opt(lhs.zinc, rhs.zinc),
        "zinc are different"
    );
}

/// Compares the product info of two products.
/// Asserts that the product info is the same.
///
/// # Arguments
/// - `lhs` - The left hand side of the comparison.
/// - `rhs` - The right hand side of the comparison.
fn compare_product_info(lhs: &ProductDescription, rhs: &ProductDescription) {
    assert_eq!(lhs.info.name, rhs.info.name);
    assert_eq!(lhs.info.id, rhs.info.id);
    assert_eq!(lhs.info.portion, rhs.info.portion);
    assert_eq!(lhs.info.producer, rhs.info.producer);
    assert_eq!(lhs.info.quantity_type, rhs.info.quantity_type);
    assert_eq!(lhs.info.volume_weight_ratio, rhs.info.volume_weight_ratio);
}

/// Compares the product requests of two products.
/// Asserts that the product requests are the same.
///
/// # Arguments
/// - `lhs` - The left hand side of the comparison.
/// - `rhs` - The right hand side of the comparison.
/// - `check_preview` - Whether to check the preview image.
fn compare_product_requests(
    lhs: &(DBId, ProductRequest),
    rhs: &(DBId, ProductRequest),
    check_preview: bool,
) {
    assert_eq!(lhs.0, rhs.0);

    let lhs = &lhs.1;
    let rhs = &rhs.1;
    assert_eq!(truncate_datetime(lhs.date), truncate_datetime(rhs.date));
    compare_product_description(
        &lhs.product_description,
        &rhs.product_description,
        check_preview,
    );
}

/// Compares the product description of two products.
/// Asserts that the product descriptions are the same.
///
/// # Arguments
/// - `lhs` - The left hand side of the comparison.
/// - `rhs` - The right hand side of the comparison.
/// - `check_preview` - Whether to check the preview image.
fn compare_product_description(
    lhs: &ProductDescription,
    rhs: &ProductDescription,
    check_preview: bool,
) {
    compare_product_info(lhs, rhs);
    check_compare_nutrients(&lhs.nutrients, &rhs.nutrients);

    if check_preview {
        assert_eq!(lhs.preview, rhs.preview);
    }
}

/// Runs the service tests with the given backend.
///
/// # Arguments
/// - `options` - The options for initializing the service.
async fn service_tests<B: DataBackend + 'static>(options: Options) {
    info!("TEST: Creating service instance...");
    let service: Arc<Service<B>> = Arc::new(Service::new(options).await.unwrap());
    let service_clone = service.clone();

    let ret = service.run();

    info!("TEST: Creating service instance...DONE");

    // spawn a task that will stop the service after 1 second
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        service_clone.stop();
    });

    ret.await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_service() {
    const SERVICE_ADDRESS: &str = "0.0.0.0:8888";

    init_logger();

    // check if the TEST_DATABASE_URL environment variable is set
    if std::env::var("TEST_DATABASE_URL").is_ok() {
        info!("TEST_DATABASE_URL has been provided, skipping docker test and using the provided database");
        let options = PostgresConfig {
            host: "localhost".to_string(),
            port: 5432,
            dbname: "postgres".to_string(),
            user: "postgres".to_string(),
            password: Secret::from_str("postgres").unwrap(),
            max_connections: 5,
        };

        let options = Options {
            postgres: options,
            address: SERVICE_ADDRESS.to_string(),
        };

        info!("Running service tests...");
        service_tests::<PostgresBackend>(options).await;
        info!("Running service tests...SUCCESS");

        return;
    }

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

        let postgres_options = PostgresConfig {
            host: "localhost".to_string(),
            port: *port as u16,
            dbname: "postgres".to_string(),
            user: "postgres".to_string(),
            password: Secret::from_str("password").unwrap(),
            max_connections: 5,
        };

        let options = Options {
            postgres: postgres_options,
            address: SERVICE_ADDRESS.to_string(),
        };

        info!("Running service tests...");
        service_tests::<PostgresBackend>(options).await;
        info!("Running service tests...SUCCESS");
    })
    .await;
}
