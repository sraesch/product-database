mod data_backend;
mod error;
mod options;
mod postgres;
mod secret;

use ::serde::{Deserialize, Serialize};
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
    pub id: ProductID,
    pub name: String,
    pub producer: Option<String>,

    /// Base64 encoded image of the form `data:image/png;base64,...`
    pub preview: Option<String>,
    pub nutrients: Nutrients,
}

/// The nutrients of a single product
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nutrients {
    pub kcal: f64,
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
    pub value: f64,
}

impl Weight {
    pub fn new_from_gram(gram: f64) -> Self {
        Self { value: gram }
    }

    pub fn new_from_milligram(milligram: f64) -> Self {
        Self {
            value: milligram * 1e-3,
        }
    }

    /// Returns the weight as gram
    pub fn gram(self) -> f64 {
        self.value
    }

    /// Returns the weight as milligram
    pub fn milligram(self) -> f64 {
        self.value * 1e3
    }

    /// Returns the weight as microgram
    pub fn microgram(self) -> f64 {
        self.value * 1e6
    }
}

/// Volume unit
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Volume {
    /// The volume expressed in litre
    pub value: f64,
}

impl Volume {
    pub fn new_from_millilitre(millilitre: f64) -> Self {
        Self {
            value: millilitre * 1e-3,
        }
    }

    /// Returns the volume as litre
    pub fn litre(self) -> f64 {
        self.value
    }

    /// Returns the volume as millilitre
    pub fn millilitre(self) -> f64 {
        self.value * 1e3
    }
}

/// The quantity in which the product details are expressed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Quantity {
    Weight(Weight),
    Volume(Volume),
}
