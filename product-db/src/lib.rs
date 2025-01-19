mod data_backend;
mod error;
mod options;
mod postgres;
mod secret;

use ::serde::{Deserialize, Serialize};
use chrono::{DateTime, Local};
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
    pub preview: Option<ProductPreview>,

    /// The nutrients of the product.
    pub nutrients: Nutrients,
}

/// The preview image of the product.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductPreview {
    #[serde(rename = "contentType")]
    /// The content type of the preview image.
    pub content_type: String,

    /// The base64 encoded image.
    #[serde_as(as = "Base64")]
    pub data: Vec<u8>,
}

/// A request to add a new product to the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductRequest {
    /// The information about the product.
    pub product_info: ProductInfo,

    /// The photo of the product, if available.
    pub product_photo: Option<Vec<u8>>,

    /// The date when the product has been requested to be added.
    pub date: DateTime<Local>,
}

/// The nutrients of a single product
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nutrients {
    pub kcal: f32,
    pub quantity: Quantity,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Quantity {
    #[serde(rename = "weight")]
    Weight(QuantityInner),

    #[serde(rename = "volume")]
    Volume(QuantityInner),
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
