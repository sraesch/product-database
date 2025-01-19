use std::future::Future;

use chrono::{DateTime, Local};
use serde_yaml::with;

use crate::{ProductID, ProductInfo, ProductRequest, Result};

pub type DBId = i32;

pub trait DataBackend: Send + Sync {
    /// Reports a missing product and returns an internal id in the database.
    ///
    /// # Arguments
    /// - `id` - The id of the missing product
    /// - `date`- The date when the product has been reported as missing.
    fn report_missing_product(
        &self,
        id: ProductID,
        date: DateTime<Local>,
    ) -> impl Future<Output = Result<DBId>> + Send;

    /// Deletes the reported missing product from the database.
    ///
    /// # Arguments
    /// - `id` - The internal id of the missing product
    fn delete_reported_missing_product(&self, id: DBId) -> impl Future<Output = Result<()>> + Send;

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
    ///
    /// # Arguments
    /// - `id` - The internal id of the requested product
    /// - `with_preview` - Whether to include the preview photo of the product in the response
    fn get_product_request(
        &self,
        id: DBId,
        with_preview: bool,
    ) -> impl Future<Output = Result<Option<ProductRequest>>> + Send;

    /// Deletes the requested product from the database.
    ///
    /// # Arguments
    /// - `id` - The internal id of the requested product
    fn delete_requested_product(&self, id: DBId) -> impl Future<Output = Result<()>> + Send;
}
