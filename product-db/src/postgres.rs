use std::future::Future;

use chrono::{DateTime, Local};
use deadpool_postgres::{Config, Pool};
use log::{error, info};
use serde::Deserialize;
use tokio_postgres::{NoTls, Row};

use crate::{
    DBId, DataBackend, Error, ProductID, ProductRequest, Result as ProductDBResult, Secret,
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
        let product_info = &requested_product.product_info;
        let product_photo = requested_product.product_photo.as_deref();
        let date = &requested_product.date;

        info!("Request new product with name: {}", product_info.name);

        let preview_data = product_info.preview.as_ref().map(|p| p.data.as_slice());
        let preview_content_type = product_info.preview.as_ref().map(|p| &p.content_type);

        let client = self.get_client().await?;
        let row = match client
            .query_1(
                "insert into requested_products (
            product_id,
            date,
            name,
            producer,
            preview,
            preview_content_type,
            photo,
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
            )

            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
            returning id;",
                &[
                    &product_info.id,
                    &date,
                    &product_info.name,
                    &product_info.producer,
                    &preview_data,
                    &preview_content_type,
                    &product_photo,
                    &product_info.nutrients.protein.map(|w| w.gram()),
                    &product_info.nutrients.fat.map(|w| w.gram()),
                    &product_info.nutrients.carbohydrates.map(|w| w.gram()),
                    &product_info.nutrients.sugar.map(|w| w.gram()),
                    &product_info.nutrients.salt.map(|w| w.gram()),
                    &product_info.nutrients.vitamin_a.map(|w| w.milligram()),
                    &product_info.nutrients.vitamin_c.map(|w| w.milligram()),
                    &product_info.nutrients.vitamin_d.map(|w| w.microgram()),
                    &product_info.nutrients.iron.map(|w| w.milligram()),
                    &product_info.nutrients.calcium.map(|w| w.milligram()),
                    &product_info.nutrients.magnesium.map(|w| w.milligram()),
                    &product_info.nutrients.sodium.map(|w| w.milligram()),
                    &product_info.nutrients.zinc.map(|w| w.milligram())
                ],
            )
            .await
        {
            Ok(row) => row,
            Err(e) => {
                error!("Failed to request new product: {}", e);
                return Err(e);
            }
        };

        let db_id: DBId = row.get(0);
        info!(
            "Requested new product with name: {} as {}",
            product_info.name, db_id
        );

        Ok(db_id)
    }

    async fn get_product_request(
        &self,
        id: DBId,
        with_preview: bool,
    ) -> ProductDBResult<Option<ProductRequest>> {
        unimplemented!()
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
