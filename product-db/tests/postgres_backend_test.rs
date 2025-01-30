use std::{collections::HashSet, env::temp_dir, str::FromStr};

use chrono::Utc;
use dockertest::{
    DockerTest, Image, LogAction, LogOptions, LogPolicy, LogSource, TestBodySpecification,
};
use log::info;
use product_db::{
    DataBackend, MissingProduct, MissingProductQuery, Nutrients, PostgresBackend, PostgresConfig,
    ProductDescription, ProductID, ProductImage, ProductQuery, ProductRequest, Secret, Sorting,
    SortingField, SortingOrder, Weight,
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
fn load_products() -> Vec<ProductDescription> {
    let product_data = include_str!("../../test_data/products.json");
    serde_json::from_str(product_data).unwrap()
}

/// Finds a product by its id.
///
/// # Arguments
/// - `products` - The list of products to search in.
/// - `id` - The id of the product to search for.
fn find_product_by_id(
    products: &[ProductDescription],
    id: ProductID,
) -> Option<&ProductDescription> {
    products.iter().find(|p| p.info.id == id)
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

/// We do some simple operations s.t. the database is not empty
/// and in its boring initial state.
/// Bringing the database in a state where we can run the tests.
///
/// # Arguments
/// - `backend` - The backend to run the tests with.
async fn simple_ops<B: DataBackend>(backend: &B) {
    let products = load_products();

    backend.new_product(&products[0]).await.unwrap();
    let req_id = backend
        .request_new_product(&ProductRequest {
            product_description: products[1].clone(),
            date: Utc::now(),
        })
        .await
        .unwrap();

    // delete both entries
    backend.delete_product(&products[0].info.id).await.unwrap();
    backend.delete_requested_product(req_id).await.unwrap();
}

/// Runs the missing product tests with the given backend.
///
/// # Arguments
/// - `backend` - The backend to run the tests with.
async fn missing_product_tests<B: DataBackend>(backend: &B) {
    // load the missing products to report and sort them by date in ascending order
    let mut products_to_report: Vec<MissingProduct> =
        serde_json::from_str(include_str!("missing_products.json")).unwrap();
    products_to_report.sort_by_key(|p| p.date);

    // insert the missing products
    let mut ids = Vec::new();
    for product in products_to_report.iter() {
        let id = backend
            .report_missing_product(product.clone())
            .await
            .unwrap();
        ids.push(id);
    }

    // make sure ids are all unique
    assert_eq!(
        HashSet::<_>::from_iter(ids.iter().cloned()).len(),
        ids.len()
    );

    // query the reported missing products
    let missing_products = backend
        .query_missing_products(&MissingProductQuery {
            limit: 40,
            offset: 0,
            product_id: None,
            order: SortingOrder::Ascending,
        })
        .await
        .unwrap();

    // check if the reported missing products are the same as the inserted ones
    assert_eq!(
        missing_products
            .iter()
            .map(|m| m.1.clone())
            .collect::<Vec<MissingProduct>>(),
        products_to_report
    );

    // use the get_missing_product method to check if the reported missing products are the same as the inserted ones
    for (id, product) in missing_products.iter() {
        let missing_product = backend.get_missing_product(*id).await.unwrap();
        assert_eq!(missing_product, Some(product.clone()));
    }

    // query the reported missing products in descending order
    let missing_products_desc = backend
        .query_missing_products(&MissingProductQuery {
            limit: 40,
            offset: 0,
            product_id: None,
            order: SortingOrder::Descending,
        })
        .await
        .unwrap();

    // check if the reported missing products are the same as the inserted ones
    assert_eq!(
        missing_products_desc
            .iter()
            .map(|m| m.1.clone())
            .collect::<Vec<MissingProduct>>(),
        products_to_report
            .iter()
            .rev()
            .cloned()
            .collect::<Vec<MissingProduct>>()
    );

    // use offset and limit to query the reported missing products
    let missing_products_offset = backend
        .query_missing_products(&MissingProductQuery {
            limit: 2,
            offset: 2,
            product_id: None,
            order: SortingOrder::Ascending,
        })
        .await
        .unwrap();

    // check if the reported missing products are the same as the inserted ones
    assert_eq!(
        missing_products_offset
            .iter()
            .map(|m| m.1.clone())
            .collect::<Vec<MissingProduct>>(),
        products_to_report[2..4].to_vec()
    );

    // query the reported missing product 'foobar' ... it should occur 3 times
    let foobar_products = backend
        .query_missing_products(&MissingProductQuery {
            limit: 40,
            offset: 0,
            product_id: Some("foobar".to_string()),
            order: SortingOrder::Descending,
        })
        .await
        .unwrap();

    assert_eq!(
        foobar_products.len(),
        3,
        "foobar_products: {:?}",
        foobar_products
    );
    assert!(foobar_products.iter().all(|p| p.1.product_id == "foobar"));

    // delete the first reported missing product
    backend
        .delete_reported_missing_product(ids[3])
        .await
        .unwrap();

    // query the reported missing product 'foobar' ... it should occur 2 times
    let foobar_products = backend
        .query_missing_products(&MissingProductQuery {
            limit: 40,
            offset: 0,
            product_id: Some("foobar".to_string()),
            order: SortingOrder::Descending,
        })
        .await
        .unwrap();

    assert_eq!(foobar_products.len(), 2);
    assert!(foobar_products.iter().all(|p| p.1.product_id == "foobar"));

    // delete the first reported missing product again ... nothing should happen
    backend
        .delete_reported_missing_product(ids[3])
        .await
        .unwrap();

    // query the reported missing product 'foobar' ... it should occur 2 times
    let foobar_products = backend
        .query_missing_products(&MissingProductQuery {
            limit: 40,
            offset: 0,
            product_id: Some("foobar".to_string()),
            order: SortingOrder::Descending,
        })
        .await
        .unwrap();

    assert_eq!(foobar_products.len(), 2);
    assert!(foobar_products.iter().all(|p| p.1.product_id == "foobar"));
}

/// Runs the product requests tests with the given backend.
///
/// # Arguments
/// - `backend` - The backend to run the tests with.
async fn product_requests_tests<B: DataBackend>(backend: &B) {
    // load the products from the test_data/products.json file
    let products = load_products();

    // request the products in the list
    let mut ids = Vec::new();
    for product_desc in products.iter() {
        let product_request = ProductRequest {
            product_description: product_desc.clone(),
            date: Utc::now(),
        };

        let id = backend.request_new_product(&product_request).await.unwrap();
        info!("Requested product with id: {}", id);

        ids.push(id);
    }

    info!("Requested products with ids: {:?}", ids);

    // make sure ids are all unique
    assert_eq!(
        HashSet::<_>::from_iter(ids.iter().cloned()).len(),
        ids.len()
    );

    // check if the requested products are the same as the inserted ones by using the get_missing_product method
    for with_preview in [true, false] {
        for (id, in_product) in ids.iter().zip(products.iter()) {
            let product_request = backend
                .get_product_request(*id, with_preview)
                .await
                .unwrap()
                .unwrap();

            let out_product = &product_request.product_description;
            compare_product_description(out_product, in_product, with_preview);

            if with_preview {
                // if the preview flag is set, we also test getting the full image of the product
                let full_image: Option<ProductImage> =
                    backend.get_product_request_image(*id).await.unwrap();
                assert_eq!(full_image, in_product.full_image);
            }
        }
    }

    // delete the first 2 requested products
    backend.delete_requested_product(ids[0]).await.unwrap();
    backend.delete_requested_product(ids[1]).await.unwrap();

    assert_eq!(
        backend.get_product_request(ids[0], true).await.unwrap(),
        None
    );
    assert_eq!(
        backend.get_product_request(ids[1], true).await.unwrap(),
        None
    );
    assert_eq!(
        backend.get_product_request(ids[0], false).await.unwrap(),
        None
    );
    assert_eq!(
        backend.get_product_request(ids[1], false).await.unwrap(),
        None
    );

    // delete the first 2 requested products again ... nothing should happen
    backend.delete_requested_product(ids[0]).await.unwrap();
    backend.delete_requested_product(ids[1]).await.unwrap();

    // check that the last requested product is still there
    for with_preview in [true, false] {
        let product_request = backend
            .get_product_request(ids[2], with_preview)
            .await
            .unwrap()
            .unwrap();

        let out_product = &product_request.product_description;
        let in_product = &products[2];

        compare_product_description(out_product, in_product, with_preview);
        if with_preview {
            // if the preview flag is set, we also test getting the full image of the product
            let full_image: Option<ProductImage> =
                backend.get_product_request_image(ids[2]).await.unwrap();
            assert_eq!(full_image, in_product.full_image);
        }
    }
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

/// Runs the product tests with the given backend.
///
/// # Arguments
/// - `backend` - The backend to run the tests with.
async fn product_tests<B: DataBackend>(backend: &B) {
    // load the products from the test_data/products.json file
    let products = load_products();

    // add the products in the list
    for product_desc in products.iter() {
        info!("Added product with id: {}", product_desc.info.id);
        assert!(backend.new_product(product_desc).await.unwrap());
        info!(
            "New product {} added from producer={}",
            product_desc.info.name,
            product_desc.info.producer.as_deref().unwrap_or("None")
        );
    }

    // check if the added products are the same as the inserted ones by using the get_missing_product method
    for with_preview in [true, false] {
        for in_product in products.iter() {
            let out_product = backend
                .get_product(&in_product.info.id, with_preview)
                .await
                .unwrap()
                .unwrap();

            compare_product_description(&out_product, in_product, with_preview);

            if with_preview {
                // if the preview flag is set, we also test getting the full image of the product
                let full_image: Option<ProductImage> = backend
                    .get_product_image(&in_product.info.id)
                    .await
                    .unwrap();
                assert_eq!(full_image, in_product.full_image);
            }
        }
    }

    // execute the querying products tests
    query_products_tests(backend, products.as_slice()).await;

    // add the products in the list again ... we should get false for all of them
    for product_desc in products.iter() {
        assert!(!backend.new_product(product_desc).await.unwrap());
    }

    // delete the first 2 products
    backend.delete_product(&products[0].info.id).await.unwrap();
    backend.delete_product(&products[1].info.id).await.unwrap();

    assert_eq!(
        backend
            .get_product(&products[0].info.id, true)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        backend
            .get_product(&products[1].info.id, true)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        backend
            .get_product(&products[0].info.id, false)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        backend
            .get_product(&products[1].info.id, false)
            .await
            .unwrap(),
        None
    );

    // delete the first 2 products again ... nothing should happen
    backend.delete_product(&products[0].info.id).await.unwrap();
    backend.delete_product(&products[1].info.id).await.unwrap();

    // check that the last added product is still there
    for with_preview in [true, false] {
        let in_product = &products[2];

        let out_product = backend
            .get_product(&in_product.info.id, with_preview)
            .await
            .unwrap()
            .unwrap();

        compare_product_description(&out_product, in_product, with_preview);

        if with_preview {
            // if the preview flag is set, we also test getting the full image of the product
            let full_image: Option<ProductImage> = backend
                .get_product_image(&in_product.info.id)
                .await
                .unwrap();
            assert_eq!(full_image, in_product.full_image);
        }
    }
}

/// Runs the query products tests with the given backend.
///
/// # Arguments
/// - `backend` - The backend to run the tests with.
/// - `products` - The products to query.
async fn query_products_tests<B: DataBackend>(backend: &B, products: &[ProductDescription]) {
    info!("Querying products tests...");

    // query all products and check if they are the same as the inserted ones
    for with_preview in [true, false] {
        let out_products: Vec<ProductDescription> = backend
            .query_products(
                &ProductQuery {
                    limit: 40,
                    offset: 0,
                    search: None,
                    sorting: None,
                },
                with_preview,
            )
            .await
            .unwrap();

        assert_eq!(out_products.len(), products.len());
        for (in_product, out_product) in products.iter().zip(out_products.iter()) {
            compare_product_description(out_product, in_product, with_preview);

            if with_preview {
                // if the preview flag is set, we also test getting the full image of the product
                let full_image: Option<ProductImage> = backend
                    .get_product_image(&in_product.info.id)
                    .await
                    .unwrap();
                assert_eq!(full_image, in_product.full_image);
            }
        }

        // test everything with a search query
        let offsets = [0, 1, 2, 3, 4];
        let limits = [1, 2, 3, 4, 5];
        let sortings = [
            None,
            Some(Sorting {
                order: SortingOrder::Ascending,
                field: SortingField::Name,
            }),
            Some(Sorting {
                order: SortingOrder::Ascending,
                field: SortingField::ProductID,
            }),
            Some(Sorting {
                order: SortingOrder::Descending,
                field: SortingField::Name,
            }),
            Some(Sorting {
                order: SortingOrder::Descending,
                field: SortingField::ProductID,
            }),
        ];

        for (offset, (limit, sorting)) in offsets.iter().zip(limits.iter().zip(sortings.iter())) {
            let out_products: Vec<ProductDescription> = backend
                .query_products(
                    &ProductQuery {
                        limit: *limit,
                        offset: *offset,
                        search: None,
                        sorting: *sorting,
                    },
                    with_preview,
                )
                .await
                .unwrap();

            // sort the input products according to the sorting
            let mut sorted_products = products.to_vec();
            if let Some(sorting) = sorting {
                match sorting.field {
                    SortingField::Name => {
                        sorted_products.sort_by_key(|p| p.info.name.clone());
                    }
                    SortingField::ProductID => {
                        sorted_products.sort_by_key(|p| p.info.id.clone());
                    }
                    _ => panic!("Unsupported sorting field"),
                }

                if sorting.order == SortingOrder::Descending {
                    sorted_products.reverse();
                }
            }

            let sorted_products = sorted_products
                .iter()
                .skip(*offset as usize)
                .take(*limit as usize)
                .cloned()
                .collect::<Vec<ProductDescription>>();

            assert_eq!(out_products.len(), sorted_products.len());
            for (in_product, out_product) in sorted_products.iter().zip(out_products.iter()) {
                compare_product_description(out_product, in_product, with_preview);

                if with_preview {
                    // if the preview flag is set, we also test getting the full image of the product
                    let full_image: Option<ProductImage> = backend
                        .get_product_image(&in_product.info.id)
                        .await
                        .unwrap();
                    assert_eq!(full_image, in_product.full_image);
                }
            }
        }

        // using a search-string query, find all alpro products
        let ret = backend
            .query_products(
                &ProductQuery {
                    offset: 0,
                    limit: 5,
                    search: Some("Alpro".to_string()),
                    sorting: Some(Sorting {
                        order: SortingOrder::Descending,
                        field: SortingField::Similarity,
                    }),
                },
                with_preview,
            )
            .await
            .unwrap();

        assert_eq!(ret.len(), 2);

        // get the two reference products
        let alpro1 = find_product_by_id(products, "5411188080213".to_string()).unwrap();
        let alpro2 = find_product_by_id(products, "5411188124689".to_string()).unwrap();
        compare_product_description(&ret[0], alpro1, with_preview);
        compare_product_description(&ret[1], alpro2, with_preview);

        if with_preview {
            // if the preview flag is set, we also test getting the full image of the product
            let full_image: Option<ProductImage> =
                backend.get_product_image(&ret[0].info.id).await.unwrap();
            assert_eq!(full_image, ret[1].full_image);
        }
    }

    info!("Querying products tests...SUCCESS");
}

/// Runs the backend tests with the given backend.
///
/// # Arguments
/// - `backend` - The backend to run the tests with.
async fn backend_tests<B: DataBackend>(backend: B) {
    info!("Do some operations with the backend...");
    simple_ops(&backend).await;
    info!("Do some operations with the backend...DONE");

    info!("Running backend tests...");
    missing_product_tests(&backend).await;
    info!("Running backend tests...SUCCESS");

    info!("Running product requests tests...");
    product_requests_tests(&backend).await;
    info!("Running product requests tests...SUCCESS");

    info!("Running product tests...");
    product_tests(&backend).await;
    info!("Running product tests...SUCCESS");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_postgres_backend() {
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

        let postgres_backend = PostgresBackend::new(options).await.unwrap();

        info!("Running backend tests...");
        backend_tests(postgres_backend).await;
        info!("Running backend tests...SUCCESS");

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

        let options = PostgresConfig {
            host: "localhost".to_string(),
            port: *port as u16,
            dbname: "postgres".to_string(),
            user: "postgres".to_string(),
            password: Secret::from_str("password").unwrap(),
            max_connections: 5,
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
