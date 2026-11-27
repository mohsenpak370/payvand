//! The request and response shapes, and the limits they are held to.

use serde::{Deserialize, Serialize};

/// Enough for a real café, small enough that the list stays a list.
pub const MAX_PRODUCTS: usize = 60;
/// Several shops are useful; a hundred is a shop-keeping system.
pub const MAX_SHOPS: usize = 20;
pub const MAX_SHOP_NAME_LEN: usize = 48;
pub const MAX_SHOP_DESCRIPTION_LEN: usize = 160;
/// Product names ride in the receipt inside the code, so they are kept short:
/// a cart's worth of them still has to fit a scannable QR.
pub const MAX_PRODUCT_NAME_LEN: usize = 32;
pub const MAX_PRODUCT_DETAILS_LEN: usize = 160;
/// A category is a heading on the menu — "Drinks", "Bakery" — not a label.
pub const MAX_PRODUCT_CATEGORY_LEN: usize = 32;
/// 10,000 SOL — a product priced above that is a typo, not a price.
pub const MAX_PRICE_LAMPORTS: u64 = 10_000_000_000_000;

/// The currencies a menu may be priced in. A stablecoin price is its own money
/// rather than a conversion of SOL: the shop charges exactly what it lists.
pub const SHOP_CURRENCIES: [&str; 3] = ["sol", "usdc", "eurc"];

pub(super) fn default_currency() -> String {
    "sol".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shop {
    pub id: String,
    pub name: String,
    pub description: String,
    /// What its menu is priced — and charged — in.
    #[serde(default = "default_currency")]
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: String,
    /// The shop this product is sold from.
    pub shop_id: String,
    pub name: String,
    /// Free text — what is in it, the size, the allergen. Never in the code.
    pub details: String,
    /// Optional heading the product is listed under, empty for no category.
    pub category: String,
    /// The price in the shop's currency's own base units — lamports for SOL,
    /// the token's atoms for USDC or EURC. The name is older than the choice of
    /// currency; what it holds is always the unit the invoice is built from.
    pub price_lamports: u64,
    pub created_at_unix: i64,
}

/// A shop with its catalogue. The profile lists several of these to count
/// products, and the payment page needs the products to price a cart.
#[derive(Debug, Serialize)]
pub struct ShopWithProducts {
    pub id: String,
    pub name: String,
    pub description: String,
    /// The currency every price below is in.
    pub currency: String,
    pub products: Vec<Product>,
}

#[derive(Debug, Deserialize)]
pub struct SaveShopRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Left out by a client that is only renaming the shop: the currency it
    /// already charges in is kept.
    #[serde(default)]
    pub currency: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SaveProductRequest {
    pub name: String,
    #[serde(default)]
    pub details: String,
    #[serde(default)]
    pub category: String,
    pub price_lamports: u64,
}

/// The category names a shop has used, for the editor's suggestions.
#[derive(Debug, Serialize)]
pub struct CategoryNames {
    pub categories: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CategoryRequest {
    pub name: String,
}
