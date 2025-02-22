use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{DBId, MissingProduct, ProductDescription, ProductID, ProductRequest};

/// The response to a request to add a new product to the database.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProductRequestResponse {
    pub message: String,
    pub date: Option<DateTime<Utc>>,
    pub id: Option<DBId>,
}

/// The response to a reported missing product.
pub type MissingProductReportResponse = ProductRequestResponse;

/// The request to report a missing product.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MissingProductReportRequest {
    pub product_id: ProductID,
}

/// The response is only a message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OnlyMessageResponse {
    pub message: String,
}

/// The query parameter for getting a product.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GetProductRequestQuery {
    #[serde(default)]
    pub with_preview: bool,

    #[serde(default)]
    pub with_full_image: bool,
}

/// The response to a request to add a new product to the database.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GetProductRequestResponse {
    pub message: String,
    pub product_request: Option<ProductRequest>,
}

/// The response to a product request query.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProductRequestQueryResponse {
    pub message: String,
    pub product_requests: Vec<(DBId, ProductRequest)>,
}

/// The response to a missing products query.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MissingProductsQueryResponse {
    pub message: String,
    pub missing_products: Vec<(DBId, MissingProduct)>,
}

/// The response to a request to add a new product to the database.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GetReportedMissingProductResponse {
    pub message: String,
    pub missing_product: Option<MissingProduct>,
}

/// The response for getting a product.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GetProductResponse {
    pub message: String,
    pub product: Option<ProductDescription>,
}

/// The response to a query for products.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProductQueryResponse {
    pub message: String,
    pub products: Vec<ProductDescription>,
}
