pub mod cart;
pub mod cart_coupon;
pub mod cart_item;
pub mod category;
pub mod checkout_session;
pub mod customer;
pub mod customer_address;
pub mod customer_group;
pub mod product_category;
pub mod product_image;
pub mod product_tag;
/// Commerce entities module
// pub mod product; // Consolidated into ../product.rs to avoid duplicates
pub mod product_variant;
pub mod variant_image;
pub mod wishlist;
pub mod wishlist_item;

// Re-export entities
pub use super::product::{Entity as Product, Model as ProductModel};
pub use cart::{CartStatus, Entity as Cart, Model as CartModel};
pub use cart_item::{Entity as CartItem, Model as CartItemModel};
pub use category::{Entity as Category, Model as CategoryModel};
pub use customer::{CustomerStatus, Entity as Customer, Model as CustomerModel};
pub use customer_address::{Entity as CustomerAddress, Model as CustomerAddressModel};
pub use customer_group::{Entity as CustomerGroup, Model as CustomerGroupModel};
pub use product_variant::{Entity as ProductVariant, Model as ProductVariantModel};
pub use wishlist::{Entity as Wishlist, Model as WishlistModel};
pub use wishlist_item::{Entity as WishlistItem, Model as WishlistItemModel};
