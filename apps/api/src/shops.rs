//! Shops and products.
//!
//! A seller keeps a list of shops on their account — a café and a bakery — and
//! each shop has its own catalogue. The payment page prices an invoice from a
//! cart of those products, and the receipt travels inside the QR code: the API
//! still stores no invoices, so a shop is only ever the menu the seller picks
//! from.
//!
//! Nothing here is on chain. A price is a number the seller edits, not state
//! anyone has to trust: the customer pays the amount the code says, at an
//! address the program derives from it.
//!
//! This file is the public surface. Implementation and tests live in the
//! `shops/` directory:
//!
//! - `shop`: listing, opening, creating and renaming shops
//! - `product`: adding, editing and removing a shop's products
//! - `category`: the headings a shop's products are listed under
//! - `store`: the ownership check and catalogue reads every route shares
//! - `validate`: the text, price and currency checks on every write
//! - `model`: the request and response shapes, and their limits

mod category;
mod model;
mod product;
mod shop;
mod store;
mod validate;

pub use category::{add_category, categories, delete_category};
pub use model::{
    CategoryNames, CategoryRequest, Product, SaveProductRequest, SaveShopRequest, Shop,
    ShopWithProducts, MAX_PRICE_LAMPORTS, MAX_PRODUCTS, MAX_PRODUCT_CATEGORY_LEN,
    MAX_PRODUCT_DETAILS_LEN, MAX_PRODUCT_NAME_LEN, MAX_SHOPS, MAX_SHOP_DESCRIPTION_LEN,
    MAX_SHOP_NAME_LEN, SHOP_CURRENCIES,
};
pub use product::{create_product, delete_product, update_product};
pub use shop::{create, get, list, update};

#[cfg(test)]
mod tests;
