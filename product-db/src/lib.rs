mod data_backend;
mod error;
mod options;
mod postgres;
mod secret;

use std::fmt::Display;

use ::serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use serde_with::{base64::Base64, serde_as};

pub use data_backend::*;
pub use error::*;
pub use options::*;
pub use postgres::*;
pub use secret::*;

/// The id of a single product
pub type ProductID = String;

/// The product info details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductInfo {
    /// The id of the product.
    /// Can be EAN, GTIN, or any other unique identifier.
    pub id: ProductID,

    /// The name of the product.
    pub name: String,

    /// The company that produces the product.
    pub producer: Option<String>,

    /// The preview image of the product.
    pub preview: Option<ProductImage>,

    /// The nutrients of the product.
    pub nutrients: Nutrients,
}

/// The description of a product.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProductDescription {
    pub id: ProductID,

    pub name: String,
    pub producer: Option<String>,

    /// The quantity type is either weight or volume.
    /// Weight in grams is used for products like flour, sugar, etc.
    /// Volume in ml is used for products like milk, water, etc.
    pub quantity_type: QuantityType,

    /// The amount for one portion of the product in grams or ml
    /// depending on the quantity type
    pub portion: f32,

    /// The ratio between volume and weight, i.e. volume(ml) = weight(g) * volume_weight_ratio
    /// Is only defined if the quantity type is volume
    pub volume_weight_ratio: Option<f32>,

    /// A preview image of the product.
    pub preview: Option<ProductImage>,

    /// The full image of the product.
    pub full_image: Option<ProductImage>,

    /// The nutrients of the product.
    pub nutrients: Nutrients,
}

/// A image of the product. Can be a preview or full image of the product.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductImage {
    #[serde(rename = "contentType")]
    /// The content type of the preview image.
    pub content_type: String,

    /// The base64 encoded image.
    #[serde_as(as = "Base64")]
    pub data: Vec<u8>,
}

/// A request to add a new product to the database.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProductRequest {
    /// The information about the product.
    pub product_description: ProductDescription,

    /// The date when the product has been requested to be added.
    pub date: DateTime<Utc>,
}

/// A missing product report.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MissingProduct {
    /// The id of the missing product.
    pub id: ProductID,

    /// The date when the product has been reported as missing.
    pub date: DateTime<Utc>,
}

/// The nutrients of a single product expressed for a reference quantity of 100g.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Nutrients {
    pub kcal: f32,

    pub protein: Option<Weight>,
    pub fat: Option<Weight>,
    pub carbohydrates: Option<Weight>,

    pub sugar: Option<Weight>,
    pub salt: Option<Weight>,

    #[serde(rename = "vitaminA")]
    pub vitamin_a: Option<Weight>,

    #[serde(rename = "vitaminC")]
    pub vitamin_c: Option<Weight>,

    #[serde(rename = "vitaminD")]
    pub vitamin_d: Option<Weight>,

    pub iron: Option<Weight>,
    pub calcium: Option<Weight>,
    pub magnesium: Option<Weight>,
    pub sodium: Option<Weight>,
    pub zinc: Option<Weight>,
}

/// Weight unit
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Weight {
    /// The weight value expressed in gram
    pub value: f32,
}

impl Weight {
    pub fn new_from_gram(gram: f32) -> Self {
        Self { value: gram }
    }

    pub fn new_from_milligram(milligram: f32) -> Self {
        Self {
            value: milligram * 1e-3,
        }
    }

    pub fn new_from_microgram(microgram: f32) -> Self {
        Self {
            value: microgram * 1e-6,
        }
    }

    /// Returns the weight as gram
    pub fn gram(self) -> f32 {
        self.value
    }

    /// Returns the weight as milligram
    pub fn milligram(self) -> f32 {
        self.value * 1e3
    }

    /// Returns the weight as microgram
    pub fn microgram(self) -> f32 {
        self.value * 1e6
    }
}

/// Volume unit
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Volume {
    /// The volume expressed in litre
    pub value: f32,
}

impl Volume {
    pub fn new_from_millilitre(millilitre: f32) -> Self {
        Self {
            value: millilitre * 1e-3,
        }
    }

    /// Returns the volume as litre
    pub fn litre(self) -> f32 {
        self.value
    }

    /// Returns the volume as millilitre
    pub fn millilitre(self) -> f32 {
        self.value * 1e3
    }
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct QuantityInnerValue {
    pub value: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct QuantityInner {
    #[serde(rename = "_0")]
    pub inner: QuantityInnerValue,
}

impl QuantityInner {
    pub fn into_weight(self) -> Weight {
        Weight {
            value: self.inner.value,
        }
    }

    pub fn into_volume(self) -> Volume {
        Volume {
            value: self.inner.value,
        }
    }
}

/// The quantity in which the product details are expressed
#[derive(
    Debug, sqlx::Type, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[sqlx(type_name = "QuantityType", rename_all = "lowercase")]
pub enum QuantityType {
    #[serde(rename = "weight")]
    Weight,

    #[serde(rename = "volume")]
    Volume,
}

impl Display for QuantityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QuantityType::Weight => write!(f, "weight"),
            QuantityType::Volume => write!(f, "volume"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_deserialize_json() {
        let product_data = include_str!("../../test_data/products.json");
        let products: Vec<ProductInfo> = serde_json::from_str(product_data).unwrap();
        assert_eq!(products.len(), 3);

        for p in products.iter() {
            if let Some(preview) = &p.preview {
                assert_eq!(preview.content_type, "image/jpeg");

                let bytes = preview.data.as_slice();
                let img = load_image::load_data(bytes).unwrap();
                assert_eq!(img.width, 128);
                assert_eq!(img.height, 128);
            }
        }
    }
}
