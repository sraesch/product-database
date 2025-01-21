use chrono::{DateTime, Utc};
use log::{debug, error, info};
use serde::Deserialize;
use sqlx::{postgres::PgPoolOptions, Executor, Row};

use crate::{
    DBId, DataBackend, Error, Nutrients, ProductDescription, ProductID, ProductImage,
    ProductRequest, Result as ProductDBResult, Secret,
};

type Pool = sqlx::PgPool;

/// Postgres based implementation of the state backend.
pub struct PostgresBackend {
    /// The sql connection pool.
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
    pub max_connections: u32,
}

impl PostgresBackend {
    /// Create a new PostgresBackend instance.
    ///
    /// # Arguments
    /// * `config` - The configuration for the postgres connection.
    pub async fn new(config: PostgresConfig) -> ProductDBResult<Self> {
        // create connection string
        let connection_string = format!(
            "postgresql://{}:{}@{}:{}/{}",
            config.user,
            config.password.secret(),
            config.host,
            config.port,
            config.dbname
        );

        // create the connection pool
        info!("Creating Postgres connection pool...");
        let pool = match PgPoolOptions::new()
            .max_connections(config.max_connections)
            .connect(&connection_string)
            .await
        {
            Ok(pool) => pool,
            Err(e) => {
                error!("Failed to create Postgres connection pool: {}", e);
                return Err(Error::DBError(Box::new(e)));
            }
        };

        info!("Creating Postgres connection pool...DONE");

        Ok(Self { pool })
    }
}

impl DataBackend for PostgresBackend {
    async fn report_missing_product(
        &self,
        id: ProductID,
        date: DateTime<Utc>,
    ) -> ProductDBResult<DBId> {
        info!(
            "Report missing product with id: {} with timestamp {}",
            id, date
        );

        let db_id: DBId = match sqlx::query_scalar("insert into reported_missing_products (product_id, date) values ($1, $2) returning id;")
        .bind(&id)
        .bind(date).fetch_one(&self.pool).await {
                Ok(row) => row,
                Err(e) => {
                    error!("Failed to report missing product: {}", e);
                    return Err(Error::DBError(Box::new(e)));
                }
            };

        info!("Reported missing product with id: {} as {}", id, db_id);

        Ok(db_id)
    }

    async fn delete_reported_missing_product(&self, id: DBId) -> ProductDBResult<()> {
        info!("Delete reported missing product with id: {}", id);

        let query = sqlx::query("delete from reported_missing_products where id = $1;").bind(id);
        if let Err(e) = self.pool.execute(query).await {
            error!("Failed to delete reported missing product: {}", e);
            return Err(Error::DBError(Box::new(e)));
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

        // create the product description entry
        let product_desc_id = self.create_product_description(product_desc).await?;

        // insert the product into the requested_products table
        let q = sqlx::query("insert into requested_products (product_description_id, date) values ($1, $2) returning id;")
            .bind(product_desc_id)
            .bind(date);

        let db_id: DBId = match self.pool.fetch_one(q).await {
            Ok(row) => row.get(0),
            Err(e) => {
                error!("Failed to request new product: {}", e);
                return Err(Error::DBError(Box::new(e)));
            }
        };

        info!(
            "Requested new product with name: {} as {}",
            product_desc.name, db_id
        );
        Ok(db_id)
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

        let query = sqlx::query("select product_id, date, name, producer, protein_grams, fat_grams, carbohydrates_grams, sugar_grams, salt_grams, vitaminA_mg, vitaminC_mg, vitaminD_Mg, iron_mg, calcium_mg, magnesium_mg, sodium_mg, zinc_mg from requested_products where id = $1;").bind(&id);
        let row = match self.pool.fetch_optional(query).await {
            Ok(row) => row,
            Err(e) => {
                error!("Failed to get product request: {}", e);
                return Err(Error::DBError(Box::new(e)));
            }
        };

        if let Some(row) = row {
            let product_id = row.try_get(0).map_err(|e| Error::DBError(Box::new(e)))?;
            let date: DateTime<Utc> = row.try_get(1).map_err(|e| Error::DBError(Box::new(e)))?;
            let name: String = row.try_get(2).map_err(|e| Error::DBError(Box::new(e)))?;
            let producer: String = row.try_get(3).map_err(|e| Error::DBError(Box::new(e)))?;
            let protein_grams: Option<f64> =
                row.try_get(4).map_err(|e| Error::DBError(Box::new(e)))?;
            let fat_grams: Option<f64> = row.try_get(5).map_err(|e| Error::DBError(Box::new(e)))?;
            let carbohydrates_grams: Option<f64> =
                row.try_get(6).map_err(|e| Error::DBError(Box::new(e)))?;
            let sugar_grams: Option<f64> =
                row.try_get(7).map_err(|e| Error::DBError(Box::new(e)))?;
            let salt_grams: Option<f64> =
                row.try_get(8).map_err(|e| Error::DBError(Box::new(e)))?;
            let vitaminA_mg: Option<f64> =
                row.try_get(9).map_err(|e| Error::DBError(Box::new(e)))?;
            let vitaminC_mg: Option<f64> =
                row.try_get(10).map_err(|e| Error::DBError(Box::new(e)))?;
            let vitaminD_mg: Option<f64> =
                row.try_get(11).map_err(|e| Error::DBError(Box::new(e)))?;
            let iron_mg: Option<f64> = row.try_get(12).map_err(|e| Error::DBError(Box::new(e)))?;
            let calcium_mg: Option<f64> =
                row.try_get(13).map_err(|e| Error::DBError(Box::new(e)))?;
            let magnesium_mg: Option<f64> =
                row.try_get(14).map_err(|e| Error::DBError(Box::new(e)))?;
            let sodium_mg: Option<f64> =
                row.try_get(15).map_err(|e| Error::DBError(Box::new(e)))?;
            let zinc_mg: Option<f64> = row.try_get(16).map_err(|e| Error::DBError(Box::new(e)))?;

            unimplemented!()
        } else {
            Ok(None)
        }
    }

    async fn delete_requested_product(&self, id: DBId) -> ProductDBResult<()> {
        info!("Delete requested product with id: {}", id);

        let q = sqlx::query("delete from requested_products where id = $1;").bind(id);

        if let Err(err) = self.pool.execute(q).await {
            error!("Failed to delete requested product: {}", err);
            return Err(Error::DBError(Box::new(err)));
        }

        info!("Deleted requested product with id: {}", id);

        Ok(())
    }
}

impl PostgresBackend {
    /// Create a new entry for the nutrients in the database.
    ///
    /// # Arguments
    /// * `nutrients` - The nutrients to create an entry for.
    async fn create_nutrients_entry(&self, nutrients: &Nutrients) -> ProductDBResult<DBId> {
        debug!("Create new entry for nutrients: {:?}", nutrients);

        let q = sqlx::query(
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
        )
        .bind(&nutrients.kcal)
        .bind(nutrients.protein.map(|w| w.gram()))
        .bind(nutrients.fat.map(|w| w.gram()))
        .bind(nutrients.carbohydrates.map(|w| w.gram()))
        .bind(nutrients.sugar.map(|w| w.gram()))
        .bind(nutrients.salt.map(|w| w.gram()))
        .bind(nutrients.vitamin_a.map(|w| w.milligram()))
        .bind(nutrients.vitamin_c.map(|w| w.milligram()))
        .bind(nutrients.vitamin_d.map(|w| w.microgram()))
        .bind(nutrients.iron.map(|w| w.milligram()))
        .bind(nutrients.calcium.map(|w| w.milligram()))
        .bind(nutrients.magnesium.map(|w| w.milligram()))
        .bind(nutrients.sodium.map(|w| w.milligram()))
        .bind(nutrients.zinc.map(|w| w.milligram()));

        let row = match self.pool.fetch_one(q).await {
            Ok(row) => row,
            Err(e) => {
                error!("Failed to create new entry for nutrients: {}", e);
                return Err(Error::DBError(Box::new(e)));
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
    /// * `image` - The product image to store.
    async fn create_image_entry(
        &self,
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

        let q = sqlx::query(
            "insert into product_image (data, content_type) values ($1, $2) returning id;",
        )
        .bind(&image.data)
        .bind(&image.content_type);

        let row = match self.pool.fetch_one(q).await {
            Ok(row) => row,
            Err(e) => {
                error!("Failed creating entry for image: {}", e);
                return Err(Error::DBError(Box::new(e)));
            }
        };

        let db_id: DBId = row.get(0);
        debug!("Create new entry for image DONE: Id={}", db_id);

        Ok(Some(db_id))
    }

    /// Create a new entry for the description of a product in the database.
    ///
    /// # Arguments
    /// * `desc` - The product description to store.
    async fn create_product_description(&self, desc: &ProductDescription) -> ProductDBResult<DBId> {
        debug!(
            "Create new product description: id={}, name={}",
            desc.id, desc.name,
        );

        let nutrients = self.create_nutrients_entry(&desc.nutrients);
        let preview = self.create_image_entry(&desc.preview);
        let full_image = self.create_image_entry(&desc.full_image);

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
        let q = sqlx::query(
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
        )
        .bind(&desc.id)
        .bind(&desc.name)
        .bind(&desc.producer)
        .bind(&desc.quantity_type)
        .bind(desc.portion)
        .bind(desc.volume_weight_ratio)
        .bind(preview)
        .bind(full_image)
        .bind(nutrients);

        let row = match self.pool.fetch_one(q).await {
            Ok(row) => row,
            Err(e) => {
                error!(
                    "Create new product description: id={}, name={}, FAILED",
                    desc.id, desc.name
                );
                return Err(Error::DBError(Box::new(e)));
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
