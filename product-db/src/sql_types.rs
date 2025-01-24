use crate::{
    DBId, MissingProduct, Nutrients, ProductDescription, ProductID, ProductImage, ProductInfo,
    ProductRequest, QuantityType, Weight,
};

use chrono::{DateTime, Utc};

/// A missing product report.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SQLMissingProduct {
    /// The internal id of the missing product.
    pub id: i32,

    /// The id of the missing product.
    pub product_id: ProductID,

    /// The date when the product has been reported as missing.
    pub date: DateTime<Utc>,
}

impl From<SQLMissingProduct> for (DBId, MissingProduct) {
    fn from(sql_missing_product: SQLMissingProduct) -> Self {
        (
            sql_missing_product.id,
            MissingProduct {
                product_id: sql_missing_product.product_id,
                date: sql_missing_product.date,
            },
        )
    }
}

/// A product request
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SQLRequestedProduct {
    pub product_id: ProductID,
    pub date: DateTime<Utc>,
    pub name: String,
    pub producer: Option<String>,
    pub quantity_type: QuantityType,
    pub portion: f32,
    pub volume_weight_ratio: Option<f32>,
    pub kcal: f32,
    pub protein_grams: Option<f32>,
    pub fat_grams: Option<f32>,
    pub carbohydrates_grams: Option<f32>,
    pub sugar_grams: Option<f32>,
    pub salt_grams: Option<f32>,
    pub vitamin_a_mg: Option<f32>,
    pub vitamin_c_mg: Option<f32>,
    pub vitamin_d_mug: Option<f32>,
    pub iron_mg: Option<f32>,
    pub calcium_mg: Option<f32>,
    pub magnesium_mg: Option<f32>,
    pub sodium_mg: Option<f32>,
    pub zinc_mg: Option<f32>,

    pub preview: Option<Vec<u8>>,
    pub preview_content_type: Option<String>,
}

impl From<&SQLRequestedProduct> for Nutrients {
    fn from(r: &SQLRequestedProduct) -> Self {
        Self {
            kcal: r.kcal,
            protein: r.protein_grams.map(Weight::new_from_gram),
            fat: r.fat_grams.map(Weight::new_from_gram),
            carbohydrates: r.carbohydrates_grams.map(Weight::new_from_gram),
            sugar: r.sugar_grams.map(Weight::new_from_gram),
            salt: r.salt_grams.map(Weight::new_from_gram),
            vitamin_a: r.vitamin_a_mg.map(Weight::new_from_milligram),
            vitamin_c: r.vitamin_c_mg.map(Weight::new_from_milligram),
            vitamin_d: r.vitamin_d_mug.map(Weight::new_from_microgram),
            iron: r.iron_mg.map(Weight::new_from_milligram),
            calcium: r.calcium_mg.map(Weight::new_from_milligram),
            magnesium: r.magnesium_mg.map(Weight::new_from_milligram),
            sodium: r.sodium_mg.map(Weight::new_from_milligram),
            zinc: r.zinc_mg.map(Weight::new_from_milligram),
        }
    }
}

impl From<SQLRequestedProduct> for ProductInfo {
    fn from(r: SQLRequestedProduct) -> Self {
        Self {
            id: r.product_id,
            name: r.name,
            producer: r.producer,
            quantity_type: r.quantity_type,
            portion: r.portion,
            volume_weight_ratio: r.volume_weight_ratio,
        }
    }
}

impl From<SQLRequestedProduct> for (Option<ProductImage>, ProductInfo) {
    fn from(r: SQLRequestedProduct) -> Self {
        let preview = r.preview.map(|p| ProductImage {
            data: p,
            content_type: r.preview_content_type.unwrap(),
        });

        (
            preview,
            ProductInfo {
                id: r.product_id,
                name: r.name,
                producer: r.producer,
                quantity_type: r.quantity_type,
                portion: r.portion,
                volume_weight_ratio: r.volume_weight_ratio,
            },
        )
    }
}

impl From<SQLRequestedProduct> for ProductDescription {
    fn from(r: SQLRequestedProduct) -> Self {
        let nutrients = (&r).into();
        let (preview, info) = r.into();

        Self {
            info,
            nutrients,
            preview,
            full_image: None,
        }
    }
}

impl From<SQLRequestedProduct> for ProductRequest {
    fn from(r: SQLRequestedProduct) -> Self {
        Self {
            date: r.date,
            product_description: r.into(),
        }
    }
}
