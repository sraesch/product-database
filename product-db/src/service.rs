use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use log::{debug, error, info, warn};
use tokio::sync::watch;
use tower_http::cors::CorsLayer;

use crate::{service_json::*, MissingProduct, MissingProductQuery, ProductID, ProductQuery};

use crate::{
    DBId, DataBackend, EndpointOptions, Error, Options, ProductDescription, ProductRequest, Result,
};

/// The central service that provides access to the product database.
pub struct Service<DB: DataBackend> {
    options: Options,
    db: Arc<DB>,
    stop_signal_receiver: watch::Receiver<i32>,
    stop_signal_sender: watch::Sender<i32>,
}

impl<DB: DataBackend + 'static> Service<DB> {
    /// Creates a new instance of the service.
    ///
    /// # Arguments
    /// - `options` - The options for the service.
    pub async fn new(options: Options) -> Result<Self> {
        // create postgres database instance
        let db = Arc::new(DB::new(&options).await?);

        // create the stop signal channel with the initial value set to running=false
        let (tx, rx) = watch::channel(0);

        Ok(Self {
            options,
            db,
            stop_signal_receiver: rx,
            stop_signal_sender: tx,
        })
    }

    /// Returns the router for the service.
    pub async fn run(&self) -> Result<()> {
        let app = Self::setup_routes(self.db.clone(), &self.options.endpoint)?;

        let rx = self.stop_signal_receiver.clone();

        let service_addr = self.options.endpoint.address.as_str();

        // create the listener on the given address
        info!("Start listening on '{}'...", service_addr);
        let listener = match tokio::net::TcpListener::bind(service_addr).await {
            Ok(listener) => listener,
            Err(e) => {
                error!("Start listening on '{}'...FAILED", service_addr);
                error!(
                    "Failed to bind to the address {} due to {}",
                    service_addr, e
                );
                return Err(Error::NetworkError(e));
            }
        };

        info!("Start listening on '{}'...OK", service_addr);

        // start the server...
        info!("Starting the server...");
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let mut rx = rx.clone();
                // wait for the signal to shutdown the server
                if let Err(err) = rx.changed().await {
                    warn!("Failed to receive the stop signal: {}", err);
                    return;
                }

                info!("Received stop signal, stopping the server...");
            })
            .await
            .map_err(|e| {
                error!("Server error: {}", e);
                Error::NetworkError(e)
            })?;

        info!("Server stopped.");

        Ok(())
    }

    /// Stops the service.
    pub fn stop(&self) {
        info!("Stopping the server...");
        if let Err(err) = self.stop_signal_sender.send(1) {
            error!("Failed to send the stop signal: {}", err);
        }
    }

    /// Sets up the routes for the service and returns the app.
    ///
    /// # Arguments
    /// - `db` - The data backend instance to use.
    /// - `endpoint_options` - The options for the endpoint.
    fn setup_routes(db: Arc<DB>, endpoint_options: &EndpointOptions) -> Result<Router> {
        // parse the CORS-origin configuration
        let allow_origins = endpoint_options
            .allow_origin
            .parse::<HeaderValue>()
            .map_err(|e| {
                error!("Failed to parse the allow-origin value: {}", e);

                Error::ConfigError(format!("Failed to parse the allow-origin value: {}", e))
            })?;

        let cors = CorsLayer::new()
            .allow_methods(vec![Method::GET, Method::POST, Method::DELETE])
            .allow_origin(allow_origins);

        let admin_app = Self::setup_admin_endpoint();
        let user_app = Self::setup_user_endpoint();

        let app = Router::new();
        let app = app.nest("/v1/admin", admin_app).nest("/v1/user", user_app);
        let app = app.layer(cors).with_state(db);

        Ok(app)
    }

    /// Sets up the admin endpoint.
    fn setup_admin_endpoint() -> Router<Arc<DB>> {
        let app = Router::new();

        app.route(
            "/product_request/{request_id}",
            delete(Self::handle_delete_product_request),
        )
        .route(
            "/product_request/{request_id}",
            get(Self::handle_get_product_request),
        )
        .route(
            "/product_request/query",
            post(Self::handle_product_request_query),
        )
        .route(
            "/product_request/{id}/image",
            get(Self::handle_get_product_request_image),
        )
        .route(
            "/missing_products/query",
            post(Self::handle_missing_products_query),
        )
        .route(
            "/missing_products/{id}",
            get(Self::handle_get_missing_product),
        )
        .route(
            "/missing_products/{id}",
            delete(Self::handle_delete_missing_product),
        )
        .route("/product", post(Self::handle_new_product))
        .route("/product/{id}", delete(Self::handle_delete_product))
    }

    /// Sets up the user endpoint.
    fn setup_user_endpoint() -> Router<Arc<DB>> {
        let app = Router::new();

        app.route("/product_request", post(Self::handle_product_request))
            .route(
                "/missing_products",
                post(Self::handle_report_missing_product),
            )
            .route("/product/{id}", get(Self::handle_get_product))
            .route("/product/query", post(Self::handle_product_query))
            .route("/product/{id}/image", get(Self::handle_get_product_image))
    }

    /// POST: Handles a requesting a new product.
    async fn handle_product_request(
        State(state): State<Arc<DB>>,
        Json(payload): Json<ProductDescription>,
    ) -> (StatusCode, Json<ProductRequestResponse>) {
        debug!("Received product request: {:?}", payload);

        let product_request = ProductRequest {
            product_description: payload,
            date: chrono::Utc::now(),
        };

        match state.request_new_product(&product_request).await {
            Ok(id) => {
                info!("Product request received successfully");
                (
                    StatusCode::CREATED,
                    Json(ProductRequestResponse {
                        message: "Product request received successfully".to_string(),
                        date: Some(product_request.date),
                        id: Some(id),
                    }),
                )
            }
            Err(err) => {
                error!("Failed to receive product request: {}", err);
                (
                    StatusCode::BAD_REQUEST,
                    Json(ProductRequestResponse {
                        message: err.to_string(),
                        date: None,
                        id: None,
                    }),
                )
            }
        }
    }

    /// POST: Handles reporting a missing product.
    async fn handle_report_missing_product(
        State(state): State<Arc<DB>>,
        Json(payload): Json<MissingProductReportRequest>,
    ) -> (StatusCode, Json<MissingProductReportResponse>) {
        debug!("Received missing product report: {:?}", payload);

        let date = chrono::Utc::now();
        let missing_product = MissingProduct {
            product_id: payload.product_id,
            date,
        };

        match state.report_missing_product(missing_product).await {
            Ok(id) => {
                info!("Received missing product report successfully");
                (
                    StatusCode::CREATED,
                    Json(MissingProductReportResponse {
                        message: "Received missing product report successfully".to_string(),
                        date: Some(date),
                        id: Some(id),
                    }),
                )
            }
            Err(err) => {
                error!("Received missing product report failed: {}", err);
                (
                    StatusCode::BAD_REQUEST,
                    Json(MissingProductReportResponse {
                        message: err.to_string(),
                        date: Some(date),
                        id: None,
                    }),
                )
            }
        }
    }

    /// DELETE: Handles deleting a requested product.
    async fn handle_delete_product_request(
        State(state): State<Arc<DB>>,
        Path(request_id): Path<DBId>,
    ) -> (StatusCode, Json<OnlyMessageResponse>) {
        debug!("Deleting product request with id={}", request_id);

        match state.delete_requested_product(request_id).await {
            Ok(()) => {
                info!("Deleting product request with id={} successful", request_id);
                (
                    StatusCode::OK,
                    Json(OnlyMessageResponse {
                        message: "Product request deleted.".to_string(),
                    }),
                )
            }
            Err(err) => {
                error!("Failed to receive product request: {}", err);
                (
                    StatusCode::BAD_REQUEST,
                    Json(OnlyMessageResponse {
                        message: err.to_string(),
                    }),
                )
            }
        }
    }

    /// GET: Handles getting a requested product.
    async fn handle_get_product_request(
        State(state): State<Arc<DB>>,
        Path(request_id): Path<DBId>,
        query: Query<GetProductRequestQuery>,
    ) -> (StatusCode, Json<GetProductRequestResponse>) {
        debug!("Get product request with id={}", request_id);

        match state
            .get_product_request(request_id, query.with_preview)
            .await
        {
            Ok(Some(mut product_request)) => {
                if query.with_full_image {
                    match state.get_product_request_image(request_id).await {
                        Ok(Some(image)) => {
                            product_request.product_description.full_image = Some(image);
                        }
                        Ok(None) => {
                            warn!("Product request with id={} has no full image", request_id);
                        }
                        Err(err) => {
                            error!("Failed to receive product request image: {}", err);
                            return (
                                StatusCode::BAD_REQUEST,
                                Json(GetProductRequestResponse {
                                    message: err.to_string(),
                                    product_request: None,
                                }),
                            );
                        }
                    }
                }

                info!("Get product request with id={} successful", request_id);
                (
                    StatusCode::OK,
                    Json(GetProductRequestResponse {
                        message: "Product request found.".to_string(),
                        product_request: Some(product_request),
                    }),
                )
            }
            Ok(None) => {
                info!("Product request with id={} not found", request_id);
                (
                    StatusCode::NOT_FOUND,
                    Json(GetProductRequestResponse {
                        message: format!("Product with id={} not found", request_id),
                        product_request: None,
                    }),
                )
            }
            Err(err) => {
                error!("Failed to receive product request: {}", err);
                (
                    StatusCode::BAD_REQUEST,
                    Json(GetProductRequestResponse {
                        message: err.to_string(),
                        product_request: None,
                    }),
                )
            }
        }
    }

    /// POST: Handles executing a product request query.
    async fn handle_product_request_query(
        State(state): State<Arc<DB>>,
        Json(query): Json<ProductQuery>,
    ) -> (StatusCode, Json<ProductRequestQueryResponse>) {
        debug!("Get product request query [Decoded]: {:?}", query);

        match state.query_product_requests(&query, true).await {
            Ok(result) => {
                info!("Product request query successful: {:?}", query);
                (
                    StatusCode::OK,
                    Json(ProductRequestQueryResponse {
                        message: "Query executed successful".to_string(),
                        product_requests: result,
                    }),
                )
            }
            Err(err) => {
                error!("Failed to receive product request: {}", err);
                (
                    StatusCode::BAD_REQUEST,
                    Json(ProductRequestQueryResponse {
                        message: err.to_string(),
                        product_requests: Vec::new(),
                    }),
                )
            }
        }
    }

    /// POST: Handles executing a product request query.
    async fn handle_missing_products_query(
        State(state): State<Arc<DB>>,
        Json(query): Json<MissingProductQuery>,
    ) -> (StatusCode, Json<MissingProductsQueryResponse>) {
        debug!("Get missing product query: {:?}", query);

        match state.query_missing_products(&query).await {
            Ok(result) => {
                info!("Missing products query successful: {:?}", query);
                (
                    StatusCode::OK,
                    Json(MissingProductsQueryResponse {
                        message: "Query executed successful".to_string(),
                        missing_products: result,
                    }),
                )
            }
            Err(err) => {
                error!("Failed to receive product request: {}", err);
                (
                    StatusCode::BAD_REQUEST,
                    Json(MissingProductsQueryResponse {
                        message: err.to_string(),
                        missing_products: Vec::new(),
                    }),
                )
            }
        }
    }

    /// GET: Handles getting reported missing product.
    async fn handle_get_missing_product(
        State(state): State<Arc<DB>>,
        Path(request_id): Path<DBId>,
    ) -> (StatusCode, Json<GetReportedMissingProductResponse>) {
        debug!("Get reported missing product with id={}", request_id);

        match state.get_missing_product(request_id).await {
            Ok(Some(missing_product)) => {
                info!(
                    "Get reported missing product with id={} successful",
                    request_id
                );
                (
                    StatusCode::OK,
                    Json(GetReportedMissingProductResponse {
                        message: "Reported missing product found.".to_string(),
                        missing_product: Some(missing_product),
                    }),
                )
            }
            Ok(None) => {
                info!("Reported missing product with id={} not found", request_id);
                (
                    StatusCode::NOT_FOUND,
                    Json(GetReportedMissingProductResponse {
                        message: format!(
                            "Reported missing product with id={} not found",
                            request_id
                        ),
                        missing_product: None,
                    }),
                )
            }
            Err(err) => {
                error!("Failed to receive reported missing product: {}", err);
                (
                    StatusCode::BAD_REQUEST,
                    Json(GetReportedMissingProductResponse {
                        message: err.to_string(),
                        missing_product: None,
                    }),
                )
            }
        }
    }

    /// DELETE: Handles deleting a reported missing product.
    async fn handle_delete_missing_product(
        State(state): State<Arc<DB>>,
        Path(report_id): Path<DBId>,
    ) -> (StatusCode, Json<OnlyMessageResponse>) {
        debug!("Deleting reported missing product with id={}", report_id);

        match state.delete_reported_missing_product(report_id).await {
            Ok(()) => {
                info!(
                    "Deleting reported missing product with id={} successful",
                    report_id
                );
                (
                    StatusCode::OK,
                    Json(OnlyMessageResponse {
                        message: "Product request deleted.".to_string(),
                    }),
                )
            }
            Err(err) => {
                error!("Failed to receive product request: {}", err);
                (
                    StatusCode::BAD_REQUEST,
                    Json(OnlyMessageResponse {
                        message: err.to_string(),
                    }),
                )
            }
        }
    }

    /// POST: Handles adding a new product.
    async fn handle_new_product(
        State(state): State<Arc<DB>>,
        Json(payload): Json<ProductDescription>,
    ) -> (StatusCode, Json<OnlyMessageResponse>) {
        debug!("Created new product: {:?}", payload);

        match state.new_product(&payload).await {
            Ok(ret) => {
                if ret {
                    info!("New product created successfully");
                    (
                        StatusCode::CREATED,
                        Json(OnlyMessageResponse {
                            message: "Product successfully created".to_string(),
                        }),
                    )
                } else {
                    error!("Product already exists: {}", payload.info);
                    (
                        StatusCode::CONFLICT,
                        Json(OnlyMessageResponse {
                            message: format!("Product with id={} already exists", payload.info.id),
                        }),
                    )
                }
            }
            Err(err) => {
                error!("Failed to add new product: {}", err);
                (
                    StatusCode::BAD_REQUEST,
                    Json(OnlyMessageResponse {
                        message: err.to_string(),
                    }),
                )
            }
        }
    }

    /// POST: Handles deleting a product.
    async fn handle_delete_product(
        State(state): State<Arc<DB>>,
        Path(product_id): Path<ProductID>,
    ) -> (StatusCode, Json<OnlyMessageResponse>) {
        debug!("Delete product: {:?}", product_id);

        match state.delete_product(&product_id).await {
            Ok(_) => {
                info!("Product deleted successfully");
                (
                    StatusCode::OK,
                    Json(OnlyMessageResponse {
                        message: "Product deleted successfully".to_string(),
                    }),
                )
            }
            Err(err) => {
                error!("Failed to delete product: {}", err);
                (
                    StatusCode::BAD_REQUEST,
                    Json(OnlyMessageResponse {
                        message: err.to_string(),
                    }),
                )
            }
        }
    }

    /// GET: Handles getting the specified product.
    async fn handle_get_product(
        State(state): State<Arc<DB>>,
        Path(product_id): Path<ProductID>,
        query: Query<GetProductRequestQuery>,
    ) -> (StatusCode, Json<GetProductResponse>) {
        debug!("Get product with id={}", product_id);

        match state.get_product(&product_id, query.with_preview).await {
            Ok(Some(mut product_description)) => {
                if query.with_full_image {
                    match state.get_product_image(&product_id).await {
                        Ok(Some(image)) => {
                            product_description.full_image = Some(image);
                        }
                        Ok(None) => {
                            warn!("Product with id={} has no full image", product_id);
                        }
                        Err(err) => {
                            error!("Failed to receive product image: {}", err);
                            return (
                                StatusCode::BAD_REQUEST,
                                Json(GetProductResponse {
                                    message: err.to_string(),
                                    product: None,
                                }),
                            );
                        }
                    }
                }

                info!("Get product with id={} successful", product_id);
                (
                    StatusCode::OK,
                    Json(GetProductResponse {
                        message: "Product found.".to_string(),
                        product: Some(product_description),
                    }),
                )
            }
            Ok(None) => {
                info!("Product with id={} not found", product_id);
                (
                    StatusCode::NOT_FOUND,
                    Json(GetProductResponse {
                        message: format!("Product with id={} not found", product_id),
                        product: None,
                    }),
                )
            }
            Err(err) => {
                error!("Failed to receive product: {}", err);
                (
                    StatusCode::BAD_REQUEST,
                    Json(GetProductResponse {
                        message: err.to_string(),
                        product: None,
                    }),
                )
            }
        }
    }

    /// POST: Handles executing a product query.
    async fn handle_product_query(
        State(state): State<Arc<DB>>,
        Json(query): Json<ProductQuery>,
    ) -> (StatusCode, Json<ProductQueryResponse>) {
        debug!("Get product query [Decoded]: {:?}", query);

        match state.query_products(&query, true).await {
            Ok(result) => {
                info!("Product query successful: {:?}", query);
                (
                    StatusCode::OK,
                    Json(ProductQueryResponse {
                        message: "Query executed successful".to_string(),
                        products: result,
                    }),
                )
            }
            Err(err) => {
                error!("Failed to process product query: {}", err);
                (
                    StatusCode::BAD_REQUEST,
                    Json(ProductQueryResponse {
                        message: err.to_string(),
                        products: Vec::new(),
                    }),
                )
            }
        }
    }

    /// GET: Handles getting the product image.
    async fn handle_get_product_image(
        State(state): State<Arc<DB>>,
        Path(product_id): Path<ProductID>,
    ) -> impl IntoResponse {
        debug!("Get product image with id={}", product_id);

        match state.get_product_image(&product_id).await {
            Ok(Some(image)) => {
                info!("Get product image with id={} successful", product_id);

                let mut headers = HeaderMap::new();
                headers.insert(
                    header::CONTENT_TYPE,
                    HeaderValue::from_str(&image.content_type).unwrap(),
                );

                Ok((headers, image.data))
            }
            Ok(None) => {
                info!("Product with id={} has no image", product_id);
                let response = Json(OnlyMessageResponse {
                    message: format!("Product with id={} has no image", product_id),
                });

                Err((StatusCode::NOT_FOUND, response))
            }
            Err(err) => {
                error!("Failed to receive product image: {}", err);
                let response = Json(OnlyMessageResponse {
                    message: err.to_string(),
                });

                Err((StatusCode::BAD_REQUEST, response))
            }
        }
    }

    /// GET: Handles getting the product request image.
    async fn handle_get_product_request_image(
        State(state): State<Arc<DB>>,
        Path(request_id): Path<DBId>,
    ) -> impl IntoResponse {
        debug!("Get product request image with id={}", request_id);

        match state.get_product_request_image(request_id).await {
            Ok(Some(image)) => {
                info!(
                    "Get product request image with id={} successful",
                    request_id
                );

                let mut headers = HeaderMap::new();
                headers.insert(
                    header::CONTENT_TYPE,
                    HeaderValue::from_str(&image.content_type).unwrap(),
                );

                Ok((headers, image.data))
            }
            Ok(None) => {
                info!("Product request with id={} has no image", request_id);
                let response = Json(OnlyMessageResponse {
                    message: format!("Product request with id={} has no image", request_id),
                });

                Err((StatusCode::NOT_FOUND, response))
            }
            Err(err) => {
                error!("Failed to receive product image: {}", err);
                let response = Json(OnlyMessageResponse {
                    message: err.to_string(),
                });

                Err((StatusCode::BAD_REQUEST, response))
            }
        }
    }
}
