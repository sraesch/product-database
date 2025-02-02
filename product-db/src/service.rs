use std::sync::Arc;

use axum::{
    extract::{Path, Query, RawQuery, State},
    http::{HeaderValue, Method, StatusCode},
    routing::{delete, get, post},
    Json, Router,
};
use log::{debug, error, info, trace, warn};
use tokio::sync::watch;
use tower_http::cors::CorsLayer;

use crate::{service_json::*, ProductQuery};

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
        let app = app.nest("/admin", admin_app).nest("/user", user_app);
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
            get(Self::handle_product_request_query),
        )
    }

    /// Sets up the user endpoint.
    fn setup_user_endpoint() -> Router<Arc<DB>> {
        let app = Router::new();

        app.route("/product_request", post(Self::handle_product_request))
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

    /// DELETE: Handles deleting a requested product.
    async fn handle_delete_product_request(
        State(state): State<Arc<DB>>,
        Path(request_id): Path<DBId>,
    ) -> (StatusCode, Json<DeletingProductRequestResponse>) {
        debug!("Deleting product request with id={}", request_id);

        match state.delete_requested_product(request_id).await {
            Ok(()) => {
                info!("Deleting product request with id={} successful", request_id);
                (
                    StatusCode::OK,
                    Json(DeletingProductRequestResponse {
                        message: "Product request deleted.".to_string(),
                    }),
                )
            }
            Err(err) => {
                error!("Failed to receive product request: {}", err);
                (
                    StatusCode::BAD_REQUEST,
                    Json(DeletingProductRequestResponse {
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

    /// GET: Handles executing a product request query.
    async fn handle_product_request_query(
        State(state): State<Arc<DB>>,
        RawQuery(query): RawQuery,
    ) -> (StatusCode, Json<ProductRequestQueryResponse>) {
        let query_string = query.unwrap_or_default();
        trace!("Get product request query [Raw]: {:?}", query_string);
        let query_string = urlencoding::decode(&query_string).unwrap_or_default();
        debug!("Get product request query [Decoded]: {:?}", query_string);

        let query: ProductQuery = match serde_qs::from_str(&query_string) {
            Ok(query) => query,
            Err(err) => {
                error!("Failed to parse product request query: {}", err);
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ProductRequestQueryResponse {
                        message: err.to_string(),
                        product_requests: Vec::new(),
                    }),
                );
            }
        };

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
}
