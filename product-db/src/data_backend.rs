use std::{
    fmt::{self, Display, Formatter},
    future::Future,
};

use serde::{Deserialize, Serialize};

use crate::{MissingProduct, ProductDescription, ProductID, ProductImage, ProductRequest, Result};

pub type DBId = i32;

/// The sorting order for the query results.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum SortingOrder {
    #[serde(rename = "asc")]
    Ascending,

    #[serde(rename = "desc")]
    Descending,
}

impl Display for SortingOrder {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            SortingOrder::Ascending => write!(f, "ASC"),
            SortingOrder::Descending => write!(f, "DESC"),
        }
    }
}

/// The query parameters for querying the missing products.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct MissingProductQuery {
    /// The offset of the query results.
    pub offset: i32,
    /// The limit of the query results.
    pub limit: i32,
    /// The product id to filter the results for (optional).
    pub product_id: Option<ProductID>,
    /// If the results are in ascending or descending order of the reported date.
    pub order: SortingOrder,
}

/// The sorting field for the query results.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum SortingField {
    /// The date when the product was reported. (Only applicable for product requests)
    #[serde(rename = "reported_date")]
    ReportedDate,

    /// The name of the product.
    #[serde(rename = "product_name")]
    Name,

    /// The ID of the product.
    #[serde(rename = "product_id")]
    ProductID,

    /// The similarity of the search result. (Only applicable if search string is provided)
    #[serde(rename = "similarity")]
    Similarity,
}

impl Display for SortingField {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            SortingField::ReportedDate => write!(f, "date"),
            SortingField::Name => write!(f, "name"),
            SortingField::ProductID => write!(f, "product_id"),
            SortingField::Similarity => write!(f, "similarity"),
        }
    }
}

/// The sorting parameters for the query results.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub struct Sorting {
    /// The order of the sorting.
    pub order: SortingOrder,

    /// The field to sort the results by.
    pub field: SortingField,
}

/// The query parameters for querying the products.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ProductQuery {
    /// The offset of the query results.
    pub offset: i32,
    /// The limit of the query results.
    pub limit: i32,
    /// The search query to filter the results for (optional).
    pub search: Option<String>,
    /// The sorting parameters for the query results (optional).
    pub sorting: Option<Sorting>,
}

pub trait DataBackend: Send + Sync {
    /// Reports a missing product and returns an internal id in the database.
    ///
    /// # Arguments
    /// - `missing_product` - The missing product to report.
    fn report_missing_product(
        &self,
        missing_product: MissingProduct,
    ) -> impl Future<Output = Result<DBId>> + Send;

    /// Queries for missing products and returns the list of missing products.
    ///
    /// # Arguments
    /// - `query` - The query parameters for the missing products
    fn query_missing_products(
        &self,
        query: &MissingProductQuery,
    ) -> impl Future<Output = Result<Vec<(DBId, MissingProduct)>>> + Send;

    /// Deletes the reported missing product from the database.
    ///
    /// # Arguments
    /// - `id` - The internal id of the missing product
    fn delete_reported_missing_product(&self, id: DBId) -> impl Future<Output = Result<()>> + Send;

    /// Retrieves the details about the missing product with the given id.
    ///
    /// # Arguments
    /// - `id` - The internal id of the missing product
    fn get_missing_product(
        &self,
        id: DBId,
    ) -> impl Future<Output = Result<Option<MissingProduct>>> + Send;

    /// Requests a new product to be added to the database and returns the internal id.
    ///
    /// # Arguments
    /// - `requested_product` - The information about the product that is requested to be added.
    fn request_new_product(
        &self,
        requested_product: &ProductRequest,
    ) -> impl Future<Output = Result<DBId>> + Send;

    /// Retrieves the details about the product request with the given id.
    /// Returns `None` if the product request does not exist.
    /// Note: The photo of the product is not included in the response.
    ///
    /// # Arguments
    /// - `id` - The internal id of the requested product
    /// - `with_preview` - Whether to include the preview photo of the product in the response
    fn get_product_request(
        &self,
        id: DBId,
        with_preview: bool,
    ) -> impl Future<Output = Result<Option<ProductRequest>>> + Send;

    /// Retrieves the full product image related to the given product request id.
    ///
    /// # Arguments
    /// - `id` - The internal id of the requested product.
    fn get_product_request_image(
        &self,
        id: DBId,
    ) -> impl Future<Output = Result<Option<ProductImage>>> + Send;

    /// Deletes the requested product from the database.
    ///
    /// # Arguments
    /// - `id` - The internal id of the requested product
    fn delete_requested_product(&self, id: DBId) -> impl Future<Output = Result<()>> + Send;

    /// Adds a new product to the database and returns true on success and false if for example
    /// the product already exists.
    ///
    /// # Arguments
    /// - `product_desc` - The description about the product to be added.
    fn new_product(
        &self,
        product_desc: &ProductDescription,
    ) -> impl Future<Output = Result<bool>> + Send;

    /// Retrieves the details about the product with the given id.
    /// Returns `None` if the product does not exist.
    /// Note: The photo of the product is not included in the response.
    ///
    /// # Arguments
    /// - `id` - The public id of the product
    /// - `with_preview` - Whether to include the preview photo of the product in the response
    fn get_product(
        &self,
        id: &ProductID,
        with_preview: bool,
    ) -> impl Future<Output = Result<Option<ProductDescription>>> + Send;

    /// Retrieves the full product image related to the given product id.
    ///
    /// # Arguments
    /// - `id` - The public id of the product.
    fn get_product_image(
        &self,
        id: &ProductID,
    ) -> impl Future<Output = Result<Option<ProductImage>>> + Send;

    /// Deletes the product from the database.
    ///
    /// # Arguments
    /// - `id` - The public id of the product.
    fn delete_product(&self, id: &ProductID) -> impl Future<Output = Result<()>> + Send;

    /// Queries for product requests and returns the list of product requests.
    ///
    /// # Arguments
    /// - `query` - The query parameters for the product requests.
    /// - `with_preview` - Whether to include the preview photo of the product in the response.
    fn query_product_requests(
        &self,
        query: &ProductQuery,
        with_preview: bool,
    ) -> impl Future<Output = Result<Vec<(DBId, ProductDescription)>>> + Send;

    /// Queries for products and returns the list of products.
    ///
    /// # Arguments
    /// - `query` - The query parameters for the products.
    /// - `with_preview` - Whether to include the preview photo of the product in the response.
    fn query_products(
        &self,
        query: &ProductQuery,
        with_preview: bool,
    ) -> impl Future<Output = Result<Vec<ProductDescription>>> + Send;
}
