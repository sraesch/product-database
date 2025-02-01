use futures::TryStreamExt;
use log::{debug, error, info, trace, LevelFilter};
use serde::Deserialize;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    ConnectOptions, Database, Executor, QueryBuilder, Row,
};

use crate::{
    sql_types::{
        SQLMissingProduct, SQLProductDescription, SQLRequestedProduct, SQLRequestedProductWithId,
    },
    DBId, DataBackend, Error, MissingProduct, MissingProductQuery, Nutrients, Options,
    ProductDescription, ProductID, ProductImage, ProductQuery, ProductRequest,
    Result as ProductDBResult, SearchFilter, Secret, SortingField,
};

type Pool = sqlx::PgPool;

/// The maximum limit for the query results.
const LIMIT_MAX: i32 = 200;

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
    async fn new(options: &Options) -> ProductDBResult<Self> {
        let pg_config = options.postgres.clone();
        Self::new(pg_config).await
    }

    async fn report_missing_product(
        &self,
        missing_product: MissingProduct,
    ) -> ProductDBResult<DBId> {
        info!(
            "Report missing product with id: {} with timestamp {}",
            missing_product.product_id, missing_product.date
        );

        let db_id: DBId = match sqlx::query_scalar("insert into reported_missing_products (product_id, date) values ($1, $2) returning id;")
        .bind(&missing_product.product_id)
        .bind(missing_product.date).fetch_one(&self.pool).await {
                Ok(row) => row,
                Err(e) => {
                    error!("Failed to report missing product: {}", e);
                    return Err(Error::DBError(Box::new(e)));
                }
            };

        info!(
            "Reported missing product with id: {} as {}",
            missing_product.product_id, db_id
        );

        Ok(db_id)
    }

    async fn query_missing_products(
        &self,
        query: &MissingProductQuery,
    ) -> ProductDBResult<Vec<(DBId, MissingProduct)>> {
        let sorting_order = query.order.to_string();

        let mut query_builder =
            QueryBuilder::new("select id, product_id, date from reported_missing_products ");

        let mut _q: String = String::new();
        if let Some(product_id) = query.product_id.as_ref() {
            query_builder.push("where product_id = ");
            query_builder.push_bind(product_id);
        }

        query_builder.push(" order by date ");
        query_builder.push(sorting_order.as_str());
        Self::add_offset_and_limit(&mut query_builder, query.offset, query.limit);

        let query = query_builder.build_query_as::<SQLMissingProduct>();
        let mut rows = query.fetch(&self.pool);
        let mut missing_products = Vec::new();
        while let Some(row) = rows
            .try_next()
            .await
            .map_err(|e| Error::DBError(Box::new(e)))?
        {
            missing_products.push((
                row.id,
                MissingProduct {
                    product_id: row.product_id,
                    date: row.date,
                },
            ));
        }

        Ok(missing_products)
    }

    async fn get_missing_product(&self, id: DBId) -> ProductDBResult<Option<MissingProduct>> {
        debug!("Get missing product with id: {}", id);

        let query = sqlx::query_as::<_, MissingProduct>(
            "select product_id, date from reported_missing_products where id = $1;",
        )
        .bind(id);

        let row = match query.fetch_optional(&self.pool).await {
            Ok(row) => row,
            Err(e) => {
                error!("Failed to get missing product: {}", e);
                return Err(Error::DBError(Box::new(e)));
            }
        };

        if let Some(row) = row {
            debug!("Found missing product with id: {}", id);
            trace!("Product: id={}, date={}", row.product_id, row.date);

            Ok(Some(row))
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

        let mut query_builder = QueryBuilder::default();
        Self::init_get_product_request_query(&mut query_builder, with_preview, false);

        query_builder.push(" where r_id = $1;");

        let query = query_builder
            .build_query_as::<SQLRequestedProduct>()
            .bind(id);

        let row = query.fetch_optional(&self.pool).await.map_err(|e| {
            error!("Failed to get product request: {}", e);
            Error::DBError(Box::new(e))
        })?;

        if row.is_none() {
            debug!("No product request with id: {}", id);
        }

        Ok(row.map(|r| {
            if !with_preview {
                trace!(
                    "Skip preview image decoding for product request with id: {}",
                    id
                );
            }

            let request: ProductRequest = r.into();

            request
        }))
    }

    async fn get_product_request_image(&self, id: DBId) -> ProductDBResult<Option<ProductImage>> {
        debug!("Get product image for product request id: {}", id);

        let query = sqlx::query_as::<_, ProductImage>(
            "select content_type, data from requested_products_full_image where r_id = $1;",
        )
        .bind(id);

        let row = query.fetch_optional(&self.pool).await.map_err(|e| {
            error!(
                "Failed to get product image for product request {}: {}",
                id, e
            );
            Error::DBError(Box::new(e))
        })?;

        if let Some(row) = row {
            Ok(Some(row))
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

        let mut query_builder = QueryBuilder::default();
        Self::init_get_product_query(&mut query_builder, with_preview);
        query_builder.push(" where product_id = $1;");
        let query = query_builder
            .build_query_as::<SQLProductDescription>()
            .bind(id);

        let row = query.fetch_optional(&self.pool).await.map_err(|e| {
            error!("Failed to get product request: {}", e);
            Error::DBError(Box::new(e))
        })?;

        if row.is_none() {
            debug!("No product request with id: {}", id);
        }

        Ok(row.map(|r| {
            if !with_preview {
                trace!(
                    "Skip preview image decoding for product request with id: {}",
                    id
                );
            }

            let request: ProductDescription = r.into();

            request
        }))
    }

    async fn get_product_image(&self, id: &ProductID) -> ProductDBResult<Option<ProductImage>> {
        debug!("Get product image for product id: {}", id);

        let query =
            sqlx::query_as::<_, ProductImage>("select pi.content_type, pi.data from product_image pi join product_description p on p.photo = pi.id where p.product_id = $1;")
                .bind(id);

        let row = query.fetch_optional(&self.pool).await.map_err(|e| {
            error!("Failed to get product image for id={}: {}", id, e);
            Error::DBError(Box::new(e))
        })?;

        if row.is_none() {
            debug!("No product image with id: {}", id);
        }

        Ok(row)
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
    ) -> ProductDBResult<Vec<(DBId, ProductRequest)>> {
        debug!("Query product requests: {:?}", query);

        // start building the sql query
        let mut query_builder = QueryBuilder::default();
        Self::init_get_product_request_query(&mut query_builder, with_preview, true);

        // add the where clause
        match &query.filter {
            SearchFilter::NoFilter => {}
            SearchFilter::ProductID(product_id) => {
                query_builder.push(" where product_id = ");
                query_builder.push_bind(product_id);
            }
            SearchFilter::Search(s) => {
                query_builder.push(" where name_producer like ");
                query_builder.push_bind(format!("%{}%", s.to_lowercase()));
            }
        }

        // add the order by clause
        if let Some(sorting) = query.sorting.as_ref() {
            query_builder.push(" order by ");

            // check if the sorting is valid
            match sorting.field {
                SortingField::Similarity => {
                    if let SearchFilter::Search(search_string) = &query.filter {
                        query_builder.push("similarity(name_producer, ");
                        query_builder.push_bind(search_string);
                        query_builder.push(") ");
                    } else {
                        return Err(Error::InvalidSortingError(sorting.field));
                    }
                }
                SortingField::ReportedDate => {
                    query_builder.push("date");
                }
                _ => {
                    query_builder.push(sorting.field.to_string());
                }
            }

            query_builder.push(" ");
            query_builder.push(sorting.order.to_string());
        }

        // add the limit and offset to the query
        Self::add_offset_and_limit(&mut query_builder, query.offset, query.limit);

        let query = query_builder.build_query_as::<SQLRequestedProductWithId>();

        let mut rows = query.fetch(&self.pool);
        let mut result: Vec<(DBId, ProductRequest)> = Vec::new();
        while let Some(row) = rows
            .try_next()
            .await
            .map_err(|e| Error::DBError(Box::new(e)))?
        {
            let db_id = row.id;
            let product_request: ProductRequest = row.into();
            result.push((db_id, product_request));
        }

        Ok(result)
    }

    async fn query_products(
        &self,
        query: &ProductQuery,
        with_preview: bool,
    ) -> ProductDBResult<Vec<ProductDescription>> {
        debug!("Query products: {:?}", query);

        // start building the sql query
        let mut query_builder = QueryBuilder::default();
        Self::init_get_product_query(&mut query_builder, with_preview);

        // create lower case search string
        let search_string = query.filter.search_string();
        let search_string = search_string.map(|s| s.to_lowercase());

        // add the where clause
        if let Some(search_string) = search_string.as_ref() {
            query_builder.push(" where name_producer like ");
            query_builder.push_bind(format!("%{}%", search_string));
        }

        // add the order by clause
        if let Some(sorting) = query.sorting.as_ref() {
            query_builder.push(" order by ");

            // check if the sorting is valid
            match sorting.field {
                SortingField::Similarity => {
                    if let Some(search_string) = search_string.as_ref() {
                        query_builder.push("similarity(name_producer, ");
                        query_builder.push_bind(search_string.to_lowercase());
                        query_builder.push(") ");
                    } else {
                        return Err(Error::InvalidSortingError(sorting.field));
                    }
                }
                SortingField::ReportedDate => {
                    return Err(Error::InvalidSortingError(sorting.field));
                }
                _ => {
                    query_builder.push(sorting.field.to_string());
                }
            }

            query_builder.push(" ");
            query_builder.push(sorting.order.to_string());
        }

        // add the limit and offset to the query
        Self::add_offset_and_limit(&mut query_builder, query.offset, query.limit);

        let query = query_builder.build_query_as::<SQLProductDescription>();

        let mut rows = query.fetch(&self.pool);
        let mut products = Vec::new();
        while let Some(row) = rows
            .try_next()
            .await
            .map_err(|e| Error::DBError(Box::new(e)))?
        {
            let product: ProductDescription = row.into();
            products.push(product);
        }

        Ok(products)
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
            vitamin_a_mg,
            vitamin_c_mg,
            vitamin_d_mug,
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
        .bind(desc.info.quantity_type)
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

    /// Add the fields of the product to the query.
    ///
    /// # Arguments
    /// * `q` - The query builder to add the fields to.
    /// * `with_preview` - Whether to include the preview image of the product in the response.
    fn init_get_product_query<DB: Database>(q: &mut QueryBuilder<'_, DB>, with_preview: bool) {
        // start building the sql query
        q.push(
            "select product_id, name, producer, quantity_type, portion, volume_weight_ratio,
        kcal, protein_grams, fat_grams, carbohydrates_grams,
        sugar_grams, salt_grams,
        vitamin_a_mg, vitamin_c_mg, vitamin_d_mug,
        iron_mg, calcium_mg, magnesium_mg, sodium_mg, zinc_mg,",
        );

        if with_preview {
            q.push("preview, preview_content_type from products_full_with_preview");
        } else {
            q.push("null as preview, null as preview_content_type from products_full");
        }
    }

    /// Initializes the query builder with a simple query to get the product request.
    ///
    /// # Arguments
    /// * `q` - The query builder to initialize.
    /// * `with_preview` - Whether to include the preview image of the product in the response.
    /// * `with_db_id` - Whether to include the database id in the response.
    fn init_get_product_request_query<DB: Database>(
        q: &mut QueryBuilder<'_, DB>,
        with_preview: bool,
        with_db_id: bool,
    ) {
        q.push(
            "select
        product_id, date, name, producer, quantity_type, portion, volume_weight_ratio,
        kcal, protein_grams, fat_grams, carbohydrates_grams,
        sugar_grams, salt_grams,
        vitamin_a_mg, vitamin_c_mg, vitamin_d_mug,
        iron_mg, calcium_mg, magnesium_mg, sodium_mg, zinc_mg,",
        );

        if with_db_id {
            q.push("r_id,");
        }

        if with_preview {
            q.push("preview, preview_content_type from requested_products_full_with_preview");
        } else {
            q.push("null as preview, null as preview_content_type from requested_products_full");
        }
    }

    fn add_offset_and_limit<'q, DB>(q: &mut QueryBuilder<'q, DB>, offset: i32, limit: i32)
    where
        DB: Database,
        i32: sqlx::Encode<'q, DB> + sqlx::Type<DB>, // Ensure i32 can be used in SQL queries
    {
        q.push(" offset ");
        q.push_bind(offset);
        q.push(" limit ");
        q.push_bind(limit.min(LIMIT_MAX));
    }
}
