use std::future::Future;

use chrono::{DateTime, Local};

use crate::{ProductID, ProductInfo, Result};

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
    ) -> impl Future<Output = Result<u64>> + Send;

    /// Deletes the reported missing product from the database.
    ///
    /// # Arguments
    /// - `id` - The internal id of the missing product
    fn delete_reported_missing_product(&self, id: u64) -> impl Future<Output = Result<()>> + Send;

    /// Requests a new product to be added to the database and returns the internal id.
    ///
    /// # Arguments
    /// - `product` - The product to be added.
    /// - `photo` - The photo of the product.
    /// - `date` - The date when the product has been requested to be added.
    fn request_new_product(
        &self,
        product_info: ProductInfo,
        product_photo: Option<Vec<u8>>,
        date: DateTime<Local>,
    ) -> impl Future<Output = Result<u64>> + Send;

    /// Deletes the requested product from the database.
    ///
    /// # Arguments
    /// - `id` - The internal id of the requested product
    fn delete_requested_product(&self, id: u64) -> impl Future<Output = Result<()>> + Send;
}
