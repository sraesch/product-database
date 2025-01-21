use std::future::Future;

use chrono::{DateTime, Local};
use deadpool_postgres::{Client, Config, Pool};
use log::{debug, error, info};
use serde::Deserialize;
use tokio_postgres::{NoTls, Row};

use crate::{
    DBId, DataBackend, Error, Nutrients, ProductDescription, ProductID, ProductImage,
    ProductRequest, Result as ProductDBResult, Secret,
};

/// Postgres based implementation of the state backend.
pub struct PostgresBackend {
    /// The postgres connection pool.
    pool: Pool,
}

/// The configuration for connecting to the postgres database.
#[derive(Clone, Debug, Deserialize)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: Secret,
    pub dbname: String,
}

impl PostgresBackend {
    /// Create a new PostgresBackend instance.
    ///
    /// # Arguments
    /// * `config` - The configuration for the postgres connection.
    pub async fn new(config: PostgresConfig) -> ProductDBResult<Self> {
        // create the connection pool configuration
        let mut pool_config = Config::new();
        pool_config.user = Some(config.user);
        pool_config.password = Some(config.password.secret().to_string());
        pool_config.dbname = Some(config.dbname);
        pool_config.host = Some(config.host);
        pool_config.port = Some(config.port);

        // create the connection pool
        info!("Creating Postgres connection pool...");
        let pool = match pool_config.create_pool(None, NoTls) {
            Ok(pool) => pool,
            Err(e) => {
                error!("Failed to create Postgres connection pool: {}", e);
                return Err(Error::DBCreatePoolError(Box::new(e)));
            }
        };

        info!("Creating Postgres connection pool...DONE");

        Ok(Self { pool })
    }

    /// Get a client from the connection pool.
    async fn get_client(&self) -> ProductDBResult<deadpool_postgres::Client> {
        self.pool
            .get()
            .await
            .map_err(|e| Error::DBPoolError(Box::new(e)))
    }
}

impl DataBackend for PostgresBackend {
    async fn report_missing_product(
        &self,
        id: ProductID,
        date: DateTime<Local>,
    ) -> ProductDBResult<DBId> {
        info!(
            "Report missing product with id: {} with timestamp {}",
            id, date
        );

        let client = self.get_client().await?;
        let row = match client
            .query_1(
            "insert into reported_missing_products (product_id, date) values ($1, $2) returning id;",
                &[&id, &date],
            )
            .await {
                Ok(row) => row,
                Err(e) => {
                    error!("Failed to report missing product: {}", e);
                    return Err(e);
                }
            };

        let db_id: DBId = row.get(0);
        info!("Reported missing product with id: {} as {}", id, db_id);

        Ok(db_id)
    }

    async fn delete_reported_missing_product(&self, id: DBId) -> ProductDBResult<()> {
        info!("Delete reported missing product with id: {}", id);

        let client = self.get_client().await?;
        if let Err(err) = client
            .execute_statement(
                "delete from reported_missing_products where id = $1;",
                &[&id],
            )
            .await
        {
            error!("Failed to delete reported missing product: {}", err);
            return Err(err);
        }

        info!("Deleted reported missing product with id: {}", id);

        Ok(())
    }

    async fn request_new_product(
        &self,
        requested_product: &ProductRequest,
    ) -> ProductDBResult<DBId> {
        let product_desc = &requested_product.product_description;
        let date = &requested_product.date;

        info!("Request new product with name: {}", product_desc.name);

        let client = self.get_client().await?;

        // create the product description entry
        let product_desc_id = self
            .create_product_description(&client, product_desc)
            .await?;

        unimplemented!()
    }

    async fn get_product_request(
        &self,
        id: DBId,
        with_preview: bool,
    ) -> ProductDBResult<Option<ProductRequest>> {
        debug!(
            "Get product request with id: {} [Preview={}]",
            id, with_preview
        );

        let client = self.get_client().await?;
        let row = match client
            .query_0_or_1(
                "select product_id, date, name, producer, protein_grams, fat_grams, carbohydrates_grams, sugar_grams, salt_grams, vitaminA_mg, vitaminC_mg, vitaminD_Mg, iron_mg, calcium_mg, magnesium_mg, sodium_mg, zinc_mg from requested_products where id = $1;",
                &[&id],
            )
            .await
        {
            Ok(row) => row,
            Err(e) => {
                error!("Failed to get product request: {}", e);
                return Err(e);
            }
        };

        if let Some(row) = row {
            let product_id: ProductID = row.get(0);
            let date: DateTime<Local> = row.get(1);
            let name: String = row.get(2);
            let producer: String = row.get(3);
            let protein_grams: Option<f64> = row.get(4);
            let fat_grams: Option<f64> = row.get(5);
            let carbohydrates_grams: Option<f64> = row.get(6);
            let sugar_grams: Option<f64> = row.get(7);
            let salt_grams: Option<f64> = row.get(8);
            let vitaminA_mg: Option<f64> = row.get(9);
            let vitaminC_mg: Option<f64> = row.get(10);
            let vitaminD_mg: Option<f64> = row.get(11);
            let iron_mg: Option<f64> = row.get(12);
            let calcium_mg: Option<f64> = row.get(13);
            let magnesium_mg: Option<f64> = row.get(14);
            let sodium_mg: Option<f64> = row.get(15);
            let zinc_mg: Option<f64> = row.get(16);

            unimplemented!()

            // let product_info = ProductInfo {
            //     id: product_id,
            //     name,
            //     producer: Some(producer),
            //     preview: None,
            //     nutrients,
            // }

            // Ok(Some(ProductRequest {
            //     product_info,
            //     product_photo: None,
            //     date,
            // }))
        } else {
            Ok(None)
        }
    }

    async fn delete_requested_product(&self, id: DBId) -> ProductDBResult<()> {
        info!("Delete requested product with id: {}", id);

        let client = self.get_client().await?;
        if let Err(err) = client
            .execute_statement("delete from requested_products where id = $1;", &[&id])
            .await
        {
            error!("Failed to delete requested product: {}", err);
            return Err(err);
        }

        info!("Deleted requested product with id: {}", id);

        Ok(())
    }
}

impl PostgresBackend {
    /// Create a new entry for the nutrients in the database.
    ///
    /// # Arguments
    /// * `client` - The postgres client to use.
    /// * `nutrients` - The nutrients to create an entry for.
    async fn create_nutrients_entry(
        &self,
        client: &Client,
        nutrients: &Nutrients,
    ) -> ProductDBResult<DBId> {
        debug!("Create new entry for nutrients: {:?}", nutrients);

        let row = match client
            .query_1(
                "insert into nutrients (
                kcal,
                protein_grams,
                fat_grams,
                carbohydrates_grams,
                sugar_grams,
                salt_grams,
                vitaminA_mg,
                vitaminC_mg,
                vitaminD_Mg,
                iron_mg,
                calcium_mg,
                magnesium_mg,
                sodium_mg,
                zinc_mg
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14) returning id;",
                &[
                    &nutrients.kcal,
                    &nutrients.protein.map(|w| w.gram()),
                    &nutrients.fat.map(|w| w.gram()),
                    &nutrients.carbohydrates.map(|w| w.gram()),
                    &nutrients.sugar.map(|w| w.gram()),
                    &nutrients.salt.map(|w| w.gram()),
                    &nutrients.vitamin_a.map(|w| w.milligram()),
                    &nutrients.vitamin_c.map(|w| w.milligram()),
                    &nutrients.vitamin_d.map(|w| w.microgram()),
                    &nutrients.iron.map(|w| w.milligram()),
                    &nutrients.calcium.map(|w| w.milligram()),
                    &nutrients.magnesium.map(|w| w.milligram()),
                    &nutrients.sodium.map(|w| w.milligram()),
                    &nutrients.zinc.map(|w| w.milligram()),
                ],
            )
            .await
        {
            Ok(row) => row,
            Err(e) => {
                error!("Failed to create new entry for nutrients: {}", e);
                return Err(e);
            }
        };

        let db_id: DBId = row.get(0);
        debug!("Create new entry for nutrients DONE: Id={}", db_id);

        Ok(db_id)
    }

    /// Create a new entry for an image of the product in the database.
    /// If the given image is None, no entry will be created and None will be returned.
    ///
    /// # Arguments
    /// * `client` - The postgres client to use.
    /// * `image` - The product image to store.
    async fn create_image_entry(
        &self,
        client: &Client,
        image: &Option<ProductImage>,
    ) -> ProductDBResult<Option<DBId>> {
        // check if an image is available and if not return None
        let image = if let Some(image) = image {
            image
        } else {
            debug!("No image available for product");
            return Ok(None);
        };

        debug!(
            "Create new entry for image: Size={}, content-type={}",
            image.data.len(),
            image.content_type
        );

        let row = match client
            .query_1(
                "insert into product_image (data, content_type) values ($1, $2) returning id;",
                &[&image.data, &image.content_type],
            )
            .await
        {
            Ok(row) => row,
            Err(e) => {
                error!("Failed creating entry for image: {}", e);
                return Err(e);
            }
        };

        let db_id: DBId = row.get(0);
        debug!("Create new entry for image DONE: Id={}", db_id);

        Ok(Some(db_id))
    }

    /// Create a new entry for the description of a product in the database.
    ///
    /// # Arguments
    /// * `client` - The postgres client to use.
    /// * `desc` - The product description to store.
    async fn create_product_description(
        &self,
        client: &Client,
        desc: &ProductDescription,
    ) -> ProductDBResult<DBId> {
        debug!(
            "Create new product description: id={}, name={}",
            desc.id, desc.name,
        );

        let nutrients = self.create_nutrients_entry(client, &desc.nutrients);
        let preview = self.create_image_entry(client, &desc.preview);
        let full_image = self.create_image_entry(client, &desc.full_image);

        // waiting for the elements nutrients, preview, and full_image to be created
        let nutrients = match nutrients.await {
            Ok(nutrients) => nutrients,
            Err(e) => {
                error!("Failed to create nutrients entry: {}", e);
                return Err(e);
            }
        };

        let preview = match preview.await {
            Ok(preview) => preview,
            Err(e) => {
                error!("Failed to create preview image entry: {}", e);
                return Err(e);
            }
        };

        let full_image = match full_image.await {
            Ok(full_image) => full_image,
            Err(e) => {
                error!("Failed to create full image entry: {}", e);
                return Err(e);
            }
        };

        // create the product description entry
        let quantity_type = desc.quantity_type.to_string();
        let row = match client
            .query_1(
                "insert into product_description (
                        product_id,
                        name,
                        producer,
                        quantity_type,
                        portion,
                        volume_weight_ratio,
                        preview,
                        photo,
                        nutrients
                ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9) returning id;",
                &[
                    &desc.id,
                    &desc.name,
                    &desc.producer,
                    &quantity_type,
                    &desc.portion,
                    &desc.volume_weight_ratio,
                    &preview,
                    &full_image,
                    &nutrients,
                ],
            )
            .await
        {
            Ok(row) => row,
            Err(e) => {
                error!(
                    "Create new product description: id={}, name={}, FAILED",
                    desc.id, desc.name
                );
                return Err(e);
            }
        };

        let db_id: DBId = row.get(0);
        debug!(
            "Create new product description: id={}, name={}, DB-Id={} DONE",
            desc.id, desc.name, db_id
        );

        Ok(db_id)
    }
}

trait SQLClientFunctionalities {
    /// Executes a SQL statement that does not return a result.
    ///
    /// # Arguments
    /// * `query` - The SQL query to execute.
    /// * `params` - The parameters to pass to the query.
    fn execute_statement(
        &self,
        query: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> impl Future<Output = ProductDBResult<u64>> + Send;

    /// Queries 0 or 1 row from the database and returns an error if there are more than 1 rows.
    ///
    /// # Arguments
    /// * `query` - The SQL select query to execute.
    /// * `params` - The parameters to pass to the query.
    fn query_0_or_1(
        &self,
        query: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> impl Future<Output = ProductDBResult<Option<Row>>> + Send;

    /// Queries 1 row from the database and returns an error if there is less or more rows.
    ///
    /// # Arguments
    /// * `query` - The SQL select query to execute.
    /// * `params` - The parameters to pass to the query.
    fn query_1(
        &self,
        query: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> impl Future<Output = ProductDBResult<Row>> + Send;

    /// Queries rows from the database.
    ///
    /// # Arguments
    /// * `query` - The SQL select query to execute.
    /// * `params` - The parameters to pass to the query.
    fn query_n(
        &self,
        query: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> impl Future<Output = ProductDBResult<Vec<Row>>> + Send;
}

impl SQLClientFunctionalities for deadpool_postgres::Client {
    /// Executes a SQL statement that does not return a result.
    ///
    /// # Arguments
    /// * `query` - The SQL query to execute.
    /// * `params` - The parameters to pass to the query.
    async fn execute_statement(
        &self,
        query: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> ProductDBResult<u64> {
        let stmt = self
            .prepare(query)
            .await
            .map_err(|e| Error::DBError(Box::new(e)))?;
        self.execute(&stmt, params)
            .await
            .map_err(|e| Error::DBError(Box::new(e)))
    }

    /// Queries 0 or 1 row from the database and returns an error if there are more than 1 rows.
    ///
    /// # Arguments
    /// * `query` - The SQL select query to execute.
    /// * `params` - The parameters to pass to the query.
    async fn query_0_or_1(
        &self,
        query: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> ProductDBResult<Option<Row>> {
        let stmt = self
            .prepare(query)
            .await
            .map_err(|e| Error::DBError(Box::new(e)))?;
        self.query_opt(&stmt, params)
            .await
            .map_err(|e| Error::DBError(Box::new(e)))
    }

    /// Queries 1 row from the database and returns an error if there is less or more rows.
    ///
    /// # Arguments
    /// * `query` - The SQL select query to execute.
    /// * `params` - The parameters to pass to the query.
    async fn query_1(
        &self,
        query: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> ProductDBResult<Row> {
        let stmt = self
            .prepare(query)
            .await
            .map_err(|e| Error::DBError(Box::new(e)))?;
        self.query_one(&stmt, params)
            .await
            .map_err(|e| Error::DBError(Box::new(e)))
    }

    /// Queries rows from the database.
    ///
    /// # Arguments
    /// * `query` - The SQL select query to execute.
    /// * `params` - The parameters to pass to the query.
    async fn query_n(
        &self,
        query: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> ProductDBResult<Vec<Row>> {
        let stmt = self
            .prepare(query)
            .await
            .map_err(|e| Error::DBError(Box::new(e)))?;

        // execute the query
        self.query(&stmt, params)
            .await
            .map_err(|e| Error::DBError(Box::new(e)))
    }
}
