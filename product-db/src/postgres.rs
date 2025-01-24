use chrono::{DateTime, Utc};
use futures::TryStreamExt;
use log::{debug, error, info, trace, LevelFilter};
use serde::Deserialize;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    ConnectOptions, Executor, Row,
};

use crate::{
    DBId, DataBackend, Error, MissingProduct, MissingProductQuery, Nutrients, ProductDescription,
    ProductID, ProductImage, ProductInfo, ProductQuery, ProductRequest, QuantityType,
    Result as ProductDBResult, Secret, Weight,
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
        // create the connection pool
        info!("Creating Postgres connection pool...");

        // get the current log level
        let log_level = log::max_level();

        let options: PgConnectOptions = PgConnectOptions::new()
            .host(&config.host)
            .port(config.port)
            .username(&config.user)
            .password(config.password.secret())
            .database(&config.dbname)
            .log_statements(if log_level == log::Level::Trace {
                LevelFilter::Trace
            } else {
                LevelFilter::Off
            });

        let pool = match PgPoolOptions::new()
            .max_connections(config.max_connections)
            .connect_with(options)
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
        missing_product: MissingProduct,
    ) -> ProductDBResult<DBId> {
        info!(
            "Report missing product with id: {} with timestamp {}",
            missing_product.id, missing_product.date
        );

        let db_id: DBId = match sqlx::query_scalar("insert into reported_missing_products (product_id, date) values ($1, $2) returning id;")
        .bind(&missing_product.id)
        .bind(missing_product.date).fetch_one(&self.pool).await {
                Ok(row) => row,
                Err(e) => {
                    error!("Failed to report missing product: {}", e);
                    return Err(Error::DBError(Box::new(e)));
                }
            };

        info!(
            "Reported missing product with id: {} as {}",
            missing_product.id, db_id
        );

        Ok(db_id)
    }

    async fn query_missing_products(
        &self,
        query: &MissingProductQuery,
    ) -> ProductDBResult<Vec<(DBId, MissingProduct)>> {
        let sorting_order = query.order.to_string();

        let mut _q: String = String::new();
        let query = if let Some(product_id) = query.product_id.as_ref() {
            _q = format!("select id, product_id, date from reported_missing_products where product_id = $1 order by date {} offset $2 limit $3;", sorting_order);
            sqlx::query(_q.as_str())
                .bind(product_id)
                .bind(query.offset)
                .bind(query.limit)
        } else {
            _q = format!("select id, product_id, date from reported_missing_products order by date {} offset $1 limit $2;", sorting_order);
            sqlx::query(_q.as_str())
                .bind(query.offset)
                .bind(query.limit)
        };

        let mut rows = self.pool.fetch(query);

        let mut missing_products = Vec::new();
        while let Some(row) = rows
            .try_next()
            .await
            .map_err(|e| Error::DBError(Box::new(e)))?
        {
            let id: DBId = row.try_get(0).map_err(|e| Error::DBError(Box::new(e)))?;
            let product_id: ProductID = row.try_get(1).map_err(|e| Error::DBError(Box::new(e)))?;
            let date: DateTime<Utc> = row.try_get(2).map_err(|e| Error::DBError(Box::new(e)))?;

            missing_products.push((
                id,
                MissingProduct {
                    id: product_id,
                    date,
                },
            ));
        }

        Ok(missing_products)
    }

    async fn get_missing_product(&self, id: DBId) -> ProductDBResult<Option<MissingProduct>> {
        debug!("Get missing product with id: {}", id);

        let query =
            sqlx::query("select product_id, date from reported_missing_products where id = $1;")
                .bind(id);
        let row = match self.pool.fetch_optional(query).await {
            Ok(row) => row,
            Err(e) => {
                error!("Failed to get missing product: {}", e);
                return Err(Error::DBError(Box::new(e)));
            }
        };

        if let Some(row) = row {
            let product_id: ProductID = row.try_get(0).map_err(|e| Error::DBError(Box::new(e)))?;
            let date: DateTime<Utc> = row.try_get(1).map_err(|e| Error::DBError(Box::new(e)))?;

            debug!("Found missing product with id: {}", id);
            trace!("Product: id={}, date={}", product_id, date);

            Ok(Some(MissingProduct {
                id: product_id,
                date,
            }))
        } else {
            debug!("No missing product with id: {}", id);
            Ok(None)
        }
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

        info!("Request new product with name: {}", product_desc.info.name);

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
            product_desc.info.name, db_id
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

        let query_str = if !with_preview {
            "select
        product_id, date, name, producer, quantity_type, portion, volume_weight_ratio,
        kcal, protein_grams, fat_grams, carbohydrates_grams,
        sugar_grams, salt_grams,
        vitaminA_mg, vitaminC_mg, vitaminD_Mg,
        iron_mg, calcium_mg, magnesium_mg, sodium_mg, zinc_mg
        from requested_products_full where r_id = $1;"
        } else {
            "select
        product_id, date, name, producer, quantity_type, portion, volume_weight_ratio,
        kcal, protein_grams, fat_grams, carbohydrates_grams,
        sugar_grams, salt_grams,
        vitaminA_mg, vitaminC_mg, vitaminD_Mg,
        iron_mg, calcium_mg, magnesium_mg, sodium_mg, zinc_mg,
        preview, preview_content_type
        from requested_products_full_with_preview where r_id = $1;"
        };

        let query = sqlx::query(query_str).bind(id);

        let row = match self.pool.fetch_optional(query).await {
            Ok(row) => row,
            Err(e) => {
                error!("Failed to get product request: {}", e);
                return Err(Error::DBError(Box::new(e)));
            }
        };

        if let Some(row) = row {
            let product_id: ProductID = row.try_get(0).map_err(|e| Error::DBError(Box::new(e)))?;
            let date: DateTime<Utc> = row.try_get(1).map_err(|e| Error::DBError(Box::new(e)))?;
            let name: String = row.try_get(2).map_err(|e| Error::DBError(Box::new(e)))?;
            let producer: Option<String> =
                row.try_get(3).map_err(|e| Error::DBError(Box::new(e)))?;
            let quantity_type: QuantityType =
                row.try_get(4).map_err(|e| Error::DBError(Box::new(e)))?;
            let portion: f32 = row.try_get(5).map_err(|e| Error::DBError(Box::new(e)))?;
            let volume_weight_ratio: Option<f32> =
                row.try_get(6).map_err(|e| Error::DBError(Box::new(e)))?;
            let kcal: f32 = row.try_get(7).map_err(|e| Error::DBError(Box::new(e)))?;
            let protein_grams: Option<f32> =
                row.try_get(8).map_err(|e| Error::DBError(Box::new(e)))?;
            let fat_grams: Option<f32> = row.try_get(9).map_err(|e| Error::DBError(Box::new(e)))?;
            let carbohydrates_grams: Option<f32> =
                row.try_get(10).map_err(|e| Error::DBError(Box::new(e)))?;
            let sugar_grams: Option<f32> =
                row.try_get(11).map_err(|e| Error::DBError(Box::new(e)))?;
            let salt_grams: Option<f32> =
                row.try_get(12).map_err(|e| Error::DBError(Box::new(e)))?;
            let vitamin_a_mg: Option<f32> =
                row.try_get(13).map_err(|e| Error::DBError(Box::new(e)))?;
            let vitamin_c_mg: Option<f32> =
                row.try_get(14).map_err(|e| Error::DBError(Box::new(e)))?;
            let vitamin_d_μg: Option<f32> =
                row.try_get(15).map_err(|e| Error::DBError(Box::new(e)))?;
            let iron_mg: Option<f32> = row.try_get(16).map_err(|e| Error::DBError(Box::new(e)))?;
            let calcium_mg: Option<f32> =
                row.try_get(17).map_err(|e| Error::DBError(Box::new(e)))?;
            let magnesium_mg: Option<f32> =
                row.try_get(18).map_err(|e| Error::DBError(Box::new(e)))?;
            let sodium_mg: Option<f32> =
                row.try_get(19).map_err(|e| Error::DBError(Box::new(e)))?;
            let zinc_mg: Option<f32> = row.try_get(20).map_err(|e| Error::DBError(Box::new(e)))?;

            let preview: Option<ProductImage> = if !with_preview {
                trace!(
                    "Skip preview image decoding for product request with id: {}",
                    id
                );
                None
            } else {
                trace!("Decode preview image for product request with id: {}", id);

                let preview_data: Option<Vec<u8>> =
                    row.try_get(21).map_err(|e| Error::DBError(Box::new(e)))?;
                let content_type: Option<String> =
                    row.try_get(22).map_err(|e| Error::DBError(Box::new(e)))?;

                trace!("Decoded preview image for product request with id: {}, Length={}, content-type={}",
                       id, preview_data.as_ref().map(|d| d.len()).unwrap_or_default(),
                       content_type.as_deref().unwrap_or_default());

                preview_data.map(|preview_data| ProductImage {
                    content_type: content_type.unwrap_or_default(),
                    data: preview_data,
                })
            };

            Ok(Some(ProductRequest {
                product_description: ProductDescription {
                    info: ProductInfo {
                        id: product_id,
                        name,
                        producer,
                        quantity_type,
                        portion,
                        volume_weight_ratio,
                    },
                    preview,
                    full_image: None,
                    nutrients: Nutrients {
                        kcal,
                        protein: protein_grams.map(Weight::new_from_gram),
                        fat: fat_grams.map(Weight::new_from_gram),
                        carbohydrates: carbohydrates_grams.map(Weight::new_from_gram),
                        sugar: sugar_grams.map(Weight::new_from_gram),
                        salt: salt_grams.map(Weight::new_from_gram),
                        vitamin_a: vitamin_a_mg.map(Weight::new_from_milligram),
                        vitamin_c: vitamin_c_mg.map(Weight::new_from_milligram),
                        vitamin_d: vitamin_d_μg.map(Weight::new_from_microgram),
                        iron: iron_mg.map(Weight::new_from_milligram),
                        calcium: calcium_mg.map(Weight::new_from_milligram),
                        magnesium: magnesium_mg.map(Weight::new_from_milligram),
                        sodium: sodium_mg.map(Weight::new_from_milligram),
                        zinc: zinc_mg.map(Weight::new_from_milligram),
                    },
                },
                date,
            }))
        } else {
            debug!("No product request with id: {}", id);
            Ok(None)
        }
    }

    async fn get_product_request_image(&self, id: DBId) -> ProductDBResult<Option<ProductImage>> {
        debug!("Get product image for product request id: {}", id);

        let query = sqlx::query(
            "select content_type, data from requested_products_full_image where r_id = $1;",
        )
        .bind(id);
        let row = match self.pool.fetch_optional(query).await {
            Ok(row) => row,
            Err(e) => {
                error!(
                    "Failed to get product image for product request {}: {}",
                    id, e
                );
                return Err(Error::DBError(Box::new(e)));
            }
        };

        if let Some(row) = row {
            let content_type: String = row.try_get(0).map_err(|e| Error::DBError(Box::new(e)))?;
            let data: Vec<u8> = row.try_get(1).map_err(|e| Error::DBError(Box::new(e)))?;

            Ok(Some(ProductImage { content_type, data }))
        } else {
            debug!("No missing product with id: {}", id);
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

    async fn new_product(&self, product_desc: &ProductDescription) -> ProductDBResult<bool> {
        info!("New product with id: {}", product_desc.info.id);

        // create the product description entry
        let product_desc_id = self.create_product_description(product_desc).await?;

        // insert the product into the products table
        let q = sqlx::query(
            "insert into products (product_description_id, product_id) values ($1, $2);",
        )
        .bind(product_desc_id)
        .bind(&product_desc.info.id);

        if let Err(err) = self.pool.execute(q).await {
            if let sqlx::Error::Database(ref db_err) = err {
                if db_err.is_unique_violation() {
                    info!(
                        "Product with id {} already exists in the database",
                        product_desc.info.id
                    );

                    // we need to cleanup the created product description entry
                    let q = sqlx::query("delete from product_description where id = $1;")
                        .bind(product_desc_id);
                    if let Err(err) = self.pool.execute(q).await {
                        error!("Failed to delete requested product: {}", err);
                        return Err(Error::DBError(Box::new(err)));
                    }

                    return Ok(false);
                } else {
                    error!(
                        "Failed to add product with id {}: {}",
                        product_desc.info.id, err
                    );
                    return Err(Error::DBError(Box::new(err)));
                }
            } else {
                error!(
                    "Failed to add product with id {}: {}",
                    product_desc.info.id, err
                );
                return Err(Error::DBError(Box::new(err)));
            }
        }

        info!("New product {} added", product_desc.info.id);

        Ok(true)
    }

    async fn get_product(
        &self,
        id: &ProductID,
        with_preview: bool,
    ) -> ProductDBResult<Option<ProductDescription>> {
        debug!("Get product with id: {} [Preview={}]", id, with_preview);

        let query_str = if !with_preview {
            "select
        product_id, name, producer, quantity_type, portion, volume_weight_ratio,
        kcal, protein_grams, fat_grams, carbohydrates_grams,
        sugar_grams, salt_grams,
        vitaminA_mg, vitaminC_mg, vitaminD_Mg,
        iron_mg, calcium_mg, magnesium_mg, sodium_mg, zinc_mg
        from products_full where product_id = $1;"
        } else {
            "select
        product_id, name, producer, quantity_type, portion, volume_weight_ratio,
        kcal, protein_grams, fat_grams, carbohydrates_grams,
        sugar_grams, salt_grams,
        vitaminA_mg, vitaminC_mg, vitaminD_Mg,
        iron_mg, calcium_mg, magnesium_mg, sodium_mg, zinc_mg,
        preview, preview_content_type
        from products_full_with_preview where product_id = $1;"
        };

        let query = sqlx::query(query_str).bind(id);

        let row = match self.pool.fetch_optional(query).await {
            Ok(row) => row,
            Err(e) => {
                error!("Failed to get product {}: {}", id, e);
                return Err(Error::DBError(Box::new(e)));
            }
        };

        if let Some(row) = row {
            let product_id: ProductID = row.try_get(0).map_err(|e| Error::DBError(Box::new(e)))?;
            let name: String = row.try_get(1).map_err(|e| Error::DBError(Box::new(e)))?;
            let producer: Option<String> =
                row.try_get(2).map_err(|e| Error::DBError(Box::new(e)))?;
            let quantity_type: QuantityType =
                row.try_get(3).map_err(|e| Error::DBError(Box::new(e)))?;
            let portion: f32 = row.try_get(4).map_err(|e| Error::DBError(Box::new(e)))?;
            let volume_weight_ratio: Option<f32> =
                row.try_get(5).map_err(|e| Error::DBError(Box::new(e)))?;
            let kcal: f32 = row.try_get(6).map_err(|e| Error::DBError(Box::new(e)))?;
            let protein_grams: Option<f32> =
                row.try_get(7).map_err(|e| Error::DBError(Box::new(e)))?;
            let fat_grams: Option<f32> = row.try_get(8).map_err(|e| Error::DBError(Box::new(e)))?;
            let carbohydrates_grams: Option<f32> =
                row.try_get(9).map_err(|e| Error::DBError(Box::new(e)))?;
            let sugar_grams: Option<f32> =
                row.try_get(10).map_err(|e| Error::DBError(Box::new(e)))?;
            let salt_grams: Option<f32> =
                row.try_get(11).map_err(|e| Error::DBError(Box::new(e)))?;
            let vitamin_a_mg: Option<f32> =
                row.try_get(12).map_err(|e| Error::DBError(Box::new(e)))?;
            let vitamin_c_mg: Option<f32> =
                row.try_get(13).map_err(|e| Error::DBError(Box::new(e)))?;
            let vitamin_d_μg: Option<f32> =
                row.try_get(14).map_err(|e| Error::DBError(Box::new(e)))?;
            let iron_mg: Option<f32> = row.try_get(15).map_err(|e| Error::DBError(Box::new(e)))?;
            let calcium_mg: Option<f32> =
                row.try_get(16).map_err(|e| Error::DBError(Box::new(e)))?;
            let magnesium_mg: Option<f32> =
                row.try_get(17).map_err(|e| Error::DBError(Box::new(e)))?;
            let sodium_mg: Option<f32> =
                row.try_get(18).map_err(|e| Error::DBError(Box::new(e)))?;
            let zinc_mg: Option<f32> = row.try_get(19).map_err(|e| Error::DBError(Box::new(e)))?;

            let preview: Option<ProductImage> = if !with_preview {
                trace!("Skip preview image decoding for product with id: {}", id);
                None
            } else {
                trace!("Decode preview image for product with id: {}", id);

                let preview_data: Option<Vec<u8>> =
                    row.try_get(20).map_err(|e| Error::DBError(Box::new(e)))?;
                let content_type: Option<String> =
                    row.try_get(21).map_err(|e| Error::DBError(Box::new(e)))?;

                trace!(
                    "Decoded preview image for product with id: {}, Length={}, content-type={}",
                    id,
                    preview_data.as_ref().map(|d| d.len()).unwrap_or_default(),
                    content_type.as_deref().unwrap_or_default()
                );

                preview_data.map(|preview_data| ProductImage {
                    content_type: content_type.unwrap_or_default(),
                    data: preview_data,
                })
            };

            Ok(Some(ProductDescription {
                info: ProductInfo {
                    id: product_id,
                    name,
                    producer,
                    quantity_type,
                    portion,
                    volume_weight_ratio,
                },
                preview,
                full_image: None,
                nutrients: Nutrients {
                    kcal,
                    protein: protein_grams.map(Weight::new_from_gram),
                    fat: fat_grams.map(Weight::new_from_gram),
                    carbohydrates: carbohydrates_grams.map(Weight::new_from_gram),
                    sugar: sugar_grams.map(Weight::new_from_gram),
                    salt: salt_grams.map(Weight::new_from_gram),
                    vitamin_a: vitamin_a_mg.map(Weight::new_from_milligram),
                    vitamin_c: vitamin_c_mg.map(Weight::new_from_milligram),
                    vitamin_d: vitamin_d_μg.map(Weight::new_from_microgram),
                    iron: iron_mg.map(Weight::new_from_milligram),
                    calcium: calcium_mg.map(Weight::new_from_milligram),
                    magnesium: magnesium_mg.map(Weight::new_from_milligram),
                    sodium: sodium_mg.map(Weight::new_from_milligram),
                    zinc: zinc_mg.map(Weight::new_from_milligram),
                },
            }))
        } else {
            debug!("No product with id: {}", id);
            Ok(None)
        }
    }

    async fn get_product_image(&self, id: &ProductID) -> ProductDBResult<Option<ProductImage>> {
        debug!("Get product image for product id: {}", id);

        let query =
            sqlx::query("select pi.content_type, pi.data from product_image pi join product_description p on p.photo = pi.id where p.product_id = $1;")
                .bind(id);
        let row = match self.pool.fetch_optional(query).await {
            Ok(row) => row,
            Err(e) => {
                error!("Failed to get product image for id={}: {}", id, e);
                return Err(Error::DBError(Box::new(e)));
            }
        };

        if let Some(row) = row {
            let content_type: String = row.try_get(0).map_err(|e| Error::DBError(Box::new(e)))?;
            let data: Vec<u8> = row.try_get(1).map_err(|e| Error::DBError(Box::new(e)))?;

            Ok(Some(ProductImage { content_type, data }))
        } else {
            debug!("No product image with id: {}", id);
            Ok(None)
        }
    }

    async fn delete_product(&self, id: &ProductID) -> ProductDBResult<()> {
        info!("Delete product with id: {}", id);

        let q = sqlx::query("delete from products where product_id = $1;").bind(id);

        if let Err(err) = self.pool.execute(q).await {
            error!("Failed to delete product: {}", err);
            return Err(Error::DBError(Box::new(err)));
        }

        info!("Deleted product with id: {}", id);

        Ok(())
    }

    async fn query_product_requests(
        &self,
        query: &ProductQuery,
        with_preview: bool,
    ) -> ProductDBResult<Vec<(DBId, ProductDescription)>> {
        unimplemented!()
    }

    async fn query_products(
        &self,
        query: &ProductQuery,
        with_preview: bool,
    ) -> ProductDBResult<Vec<ProductDescription>> {
        unimplemented!()
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
        .bind(nutrients.kcal)
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
            desc.info.id, desc.info.name,
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
        .bind(&desc.info.id)
        .bind(&desc.info.name)
        .bind(&desc.info.producer)
        .bind(&desc.info.quantity_type)
        .bind(desc.info.portion)
        .bind(desc.info.volume_weight_ratio)
        .bind(preview)
        .bind(full_image)
        .bind(nutrients);

        let row = match self.pool.fetch_one(q).await {
            Ok(row) => row,
            Err(e) => {
                error!(
                    "Create new product description: id={}, name={}, FAILED",
                    desc.info.id, desc.info.name
                );
                return Err(Error::DBError(Box::new(e)));
            }
        };

        let db_id: DBId = row.get(0);
        debug!(
            "Create new product description: id={}, name={}, DB-Id={} DONE",
            desc.info.id, desc.info.name, db_id
        );

        Ok(db_id)
    }
}
