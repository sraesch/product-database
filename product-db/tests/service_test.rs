use std::{collections::HashSet, env::temp_dir, str::FromStr, sync::Arc};

use chrono::{DateTime, Utc};
use dockertest::{
    DockerTest, Image, LogAction, LogOptions, LogPolicy, LogSource, TestBodySpecification,
};
use log::{debug, info};
use product_db::{
    service_json::*, DBId, DataBackend, EndpointOptions, Nutrients, Options, PostgresBackend,
    PostgresConfig, ProductDescription, ProductID, ProductQuery, ProductRequest, SearchFilter,
    Secret, Service, Sorting, SortingField, SortingOrder, Weight,
};
use reqwest::{StatusCode, Url};

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

/// Finds a product request by the product id.
///
/// # Arguments
/// - `product_requests` - The list of product requests to search in.
/// - `id` - The id of the product to search for its request.
fn find_product_request_by_id(
    product_requests: &[(DBId, ProductRequest)],
    id: ProductID,
) -> Option<&(DBId, ProductRequest)> {
    product_requests
        .iter()
        .find(|p| p.1.product_description.info.id == id)
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

/// Simple client to talk to the service.
pub struct ServiceClient {
    server_address: Url,
    client: reqwest::Client,
}

impl ServiceClient {
    pub fn new(server_address: String) -> Self {
        let server_address = Url::parse(&format!("http://{}/v1/", server_address)).unwrap();

        Self {
            server_address,
            client: reqwest::Client::new(),
        }
    }

    /// Creates a new product request.
    ///
    /// # Arguments
    /// - `product_description` - The product request to create.
    pub async fn request_new_product(
        &self,
        product_description: &ProductDescription,
    ) -> (DBId, DateTime<Utc>) {
        let url = self.server_address.join("user/product_request").unwrap();
        debug!("POST: {}", url);

        let response = self
            .client
            .post(url)
            .json(product_description)
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        let response: ProductRequestResponse = response.json().await.unwrap();

        (response.id.unwrap(), response.date.unwrap())
    }

    /// Gets the product request with the given id.
    ///
    /// # Arguments
    /// - `id` - The id of the product request to get.
    /// - `with_preview` - Whether to include the preview image in the response.
    /// - `with_full_image` - Whether to include the full image in the response.
    pub async fn get_product_request(
        &self,
        id: DBId,
        with_preview: bool,
        with_full_image: bool,
    ) -> Option<ProductRequest> {
        let mut url = self
            .server_address
            .join("admin/product_request/")
            .unwrap()
            .join(&id.to_string())
            .unwrap();

        if with_preview {
            url.query_pairs_mut().append_pair("with_preview", "true");
        }

        if with_full_image {
            url.query_pairs_mut().append_pair("with_full_image", "true");
        }

        debug!("GET: {}", url);

        let response = self.client.get(url).send().await.unwrap();
        debug!(
            "Product request response: status={}, length={}",
            response.status(),
            response.content_length().unwrap_or_default()
        );
        let status_code = response.status();
        assert!(status_code == StatusCode::NOT_FOUND || status_code == StatusCode::OK);
        let response: GetProductRequestResponse = response.json().await.unwrap();

        debug!("Product request response: {:?}", response);

        if status_code == StatusCode::NOT_FOUND {
            return None;
        }

        if status_code == StatusCode::NOT_FOUND {
            return None;
        }

        assert_eq!(status_code, StatusCode::OK);

        response.product_request
    }

    /// Queries the product requests.
    ///
    /// # Arguments
    /// - `query` - The query to use.
    pub async fn query_product_requests(
        &self,
        query: &ProductQuery,
    ) -> Vec<(DBId, ProductRequest)> {
        let url = self
            .server_address
            .join("admin/product_request/query")
            .unwrap();

        debug!("POST: {}", url);
        let response = self.client.post(url).json(query).send().await.unwrap();
        debug!(
            "Product request response: status={}, length={}",
            response.status(),
            response.content_length().unwrap_or_default()
        );
        let status_code = response.status();
        assert_eq!(status_code, StatusCode::OK);

        let response: ProductRequestQueryResponse = response.json().await.unwrap();

        response.product_requests
    }

    /// Deletes the product request with the given id.
    ///
    /// # Arguments
    /// - `id` - The id of the product request to get.
    pub async fn delete_requested_product(&self, id: DBId) {
        let url = self
            .server_address
            .join("admin/product_request/")
            .unwrap()
            .join(&id.to_string())
            .unwrap();

        debug!("DELETE: {}", url);

        let response = self.client.delete(url).send().await.unwrap();
        debug!(
            "Delete product request response: status={}, length={}",
            response.status(),
            response.content_length().unwrap_or_default()
        );
        let status_code = response.status();
        assert_eq!(status_code, StatusCode::OK);
        let response: OnlyMessageResponse = response.json().await.unwrap();

        debug!("Delete product request response: {:?}", response);
    }
}

/// Runs the product requests tests against the service.
///
/// # Arguments
/// - `options` - The endpoint options.
async fn product_requests_tests(options: &EndpointOptions) {
    let client = ServiceClient::new(options.address.clone());

    // load the products from the test_data/products.json file
    let products = load_products();

    // request the products in the list
    let mut ids = Vec::new();
    let mut product_requests: Vec<ProductRequest> = Vec::new();
    let mut product_requests_with_ids = Vec::new();
    for product_desc in products.iter() {
        let (id, date) = client.request_new_product(product_desc).await;
        info!("Requested product with id: {}", id);

        ids.push(id);
        product_requests.push(ProductRequest {
            date,
            product_description: product_desc.clone(),
        });

        product_requests_with_ids.push((id, product_requests.last().unwrap().clone()));
    }

    info!("Requested products with ids: {:?}", ids);

    // make sure ids are all unique
    assert_eq!(
        HashSet::<_>::from_iter(ids.iter().cloned()).len(),
        ids.len()
    );

    // check if the requested products are the same as the inserted ones by using the get_product_request method
    for with_preview in [true, false] {
        for (id, in_product) in ids.iter().zip(products.iter()) {
            let product_request = client
                .get_product_request(*id, with_preview, with_preview)
                .await
                .unwrap();

            let out_product = &product_request.product_description;
            compare_product_description(out_product, in_product, with_preview);

            if with_preview {
                // if the preview flag is set, we also test getting the full image of the product
                assert_eq!(
                    product_request.product_description.full_image,
                    in_product.full_image
                );
            }
        }
    }

    // execute the querying product requests tests
    query_product_requests_tests(&client, product_requests_with_ids.as_slice()).await;

    // add the first product request again, but modify it slightly
    let mut modified_product_request = product_requests[0].clone();
    modified_product_request.product_description.info.name += "Modified Name";
    ids.push(
        client
            .request_new_product(&modified_product_request.product_description)
            .await
            .0,
    );

    // now query the modified product request
    let product_requests = client
        .query_product_requests(&ProductQuery {
            limit: 40,
            offset: 0,
            filter: SearchFilter::ProductID(
                modified_product_request.product_description.info.id.clone(),
            ),
            sorting: None,
        })
        .await;

    assert_eq!(product_requests.len(), 2);
    assert_eq!(product_requests[0].0, ids[0]);
    assert_eq!(product_requests[1].0, ids[ids.len() - 1]);

    // delete the first 2 requested products
    client.delete_requested_product(ids[0]).await;
    client.delete_requested_product(ids[1]).await;

    assert_eq!(client.get_product_request(ids[0], true, false).await, None);
    assert_eq!(client.get_product_request(ids[1], true, false).await, None);
    assert_eq!(client.get_product_request(ids[0], false, false).await, None);
    assert_eq!(client.get_product_request(ids[1], false, false).await, None);

    // delete the first 2 requested products again ... nothing should happen
    client.delete_requested_product(ids[0]).await;
    client.delete_requested_product(ids[1]).await;

    // check that the last requested product is still there
    for with_preview in [true, false] {
        let product_request = client
            .get_product_request(ids[2], with_preview, with_preview)
            .await
            .unwrap();

        let out_product = &product_request.product_description;
        let in_product = &products[2];

        compare_product_description(out_product, in_product, with_preview);
        if with_preview {
            // if the preview flag is set, we also test getting the full image of the product
            assert_eq!(
                product_request.product_description.full_image,
                in_product.full_image
            );
        }
    }
}

/// Runs the query product requests tests.
///
/// # Arguments
/// - `client` - The service client.
/// - `product_requests` - The product requests to query.
async fn query_product_requests_tests(
    client: &ServiceClient,
    product_requests: &[(DBId, ProductRequest)],
) {
    info!("Querying product requests tests...");

    // query all product requests and check if they are the same as the inserted ones
    for with_preview in [true, false] {
        let out_products: Vec<(DBId, ProductRequest)> = client
            .query_product_requests(&ProductQuery {
                limit: 40,
                offset: 0,
                filter: SearchFilter::NoFilter,
                sorting: None,
            })
            .await;

        assert_eq!(out_products.len(), product_requests.len());
        for ((in_id, in_product), (out_id, out_product)) in
            product_requests.iter().zip(out_products.iter())
        {
            compare_product_description(
                &out_product.product_description,
                &in_product.product_description,
                with_preview,
            );
            assert_eq!(
                truncate_datetime(out_product.date),
                truncate_datetime(in_product.date)
            );
            assert_eq!(in_id, out_id);
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
                order: SortingOrder::Ascending,
                field: SortingField::ReportedDate,
            }),
            Some(Sorting {
                order: SortingOrder::Descending,
                field: SortingField::Name,
            }),
            Some(Sorting {
                order: SortingOrder::Descending,
                field: SortingField::ProductID,
            }),
            Some(Sorting {
                order: SortingOrder::Descending,
                field: SortingField::ReportedDate,
            }),
        ];

        for (offset, (limit, sorting)) in offsets.iter().zip(limits.iter().zip(sortings.iter())) {
            let out_products: Vec<(DBId, ProductRequest)> = client
                .query_product_requests(&ProductQuery {
                    limit: *limit,
                    offset: *offset,
                    filter: SearchFilter::NoFilter,
                    sorting: *sorting,
                })
                .await;

            // sort the input products according to the sorting
            let mut sorted_product_requests = product_requests.to_vec();
            if let Some(sorting) = sorting {
                match sorting.field {
                    SortingField::Name => {
                        sorted_product_requests
                            .sort_by_key(|p| p.1.product_description.info.name.clone());
                    }
                    SortingField::ProductID => {
                        sorted_product_requests
                            .sort_by_key(|p| p.1.product_description.info.id.clone());
                    }
                    SortingField::ReportedDate => {
                        sorted_product_requests.sort_by_key(|p| p.1.date);
                    }
                    _ => panic!("Unsupported sorting field"),
                }

                if sorting.order == SortingOrder::Descending {
                    sorted_product_requests.reverse();
                }
            }

            let sorted_product_requests = sorted_product_requests
                .iter()
                .skip(*offset as usize)
                .take(*limit as usize)
                .cloned()
                .collect::<Vec<(DBId, ProductRequest)>>();

            assert_eq!(out_products.len(), sorted_product_requests.len());
            for ((in_id, in_product), (out_id, out_product)) in
                sorted_product_requests.iter().zip(out_products.iter())
            {
                compare_product_description(
                    &out_product.product_description,
                    &in_product.product_description,
                    with_preview,
                );
                assert_eq!(
                    truncate_datetime(out_product.date),
                    truncate_datetime(in_product.date)
                );
                assert_eq!(in_id, out_id);
            }
        }

        // using a search-string query, find all alpro products
        let ret = client
            .query_product_requests(&ProductQuery {
                offset: 0,
                limit: 5,
                filter: SearchFilter::Search("Alpro".to_string()),
                sorting: Some(Sorting {
                    order: SortingOrder::Descending,
                    field: SortingField::Similarity,
                }),
            })
            .await;

        assert_eq!(ret.len(), 2);

        // get the two reference product requests
        let alpro1 =
            find_product_request_by_id(product_requests, "5411188080213".to_string()).unwrap();
        let alpro2 =
            find_product_request_by_id(product_requests, "5411188124689".to_string()).unwrap();
        compare_product_requests(&ret[0], alpro1, with_preview);
        compare_product_requests(&ret[1], alpro2, with_preview);
    }

    info!("Querying product requests tests...SUCCESS");
}

/// Runs the service tests with the given backend.
///
/// # Arguments
/// - `options` - The options for initializing the service.
async fn service_tests<B: DataBackend + 'static>(options: Options) {
    let endpoint_options = options.endpoint.clone();

    info!("TEST: Creating service instance...");
    let service: Arc<Service<B>> = Arc::new(Service::new(options).await.unwrap());
    let service_clone = service.clone();

    let ret = service.run();

    info!("TEST: Creating service instance...DONE");

    // spawn a task that will stop the service after 1 second
    tokio::spawn(async move {
        info!("Running product requests tests...");
        product_requests_tests(&endpoint_options).await;
        info!("Running product requests tests...SUCCESS");

        service_clone.stop();
    });

    ret.await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_service() {
    const SERVICE_ADDRESS: &str = "0.0.0.0:8888";

    let endpoint_options = EndpointOptions {
        address: SERVICE_ADDRESS.to_string(),
        ..Default::default()
    };

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
            endpoint: endpoint_options,
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
            endpoint: endpoint_options,
        };

        info!("Running service tests...");
        service_tests::<PostgresBackend>(options).await;
        info!("Running service tests...SUCCESS");
    })
    .await;
}
