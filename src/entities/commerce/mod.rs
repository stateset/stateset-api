/// Commerce entities module
// pub mod product; // Consolidated into ../product.rs to avoid duplicates
pub mod product_variant;
pub mod product_category;
pub mod product_tag;
pub mod product_image;
pub mod variant_image;
pub mod cart;
pub mod cart_item;
pub mod cart_coupon;
pub mod customer;
pub mod customer_group;
pub mod customer_address;
pub mod wishlist;
pub mod wishlist_item;
pub mod category;
pub mod checkout_session;

// Re-export entities
pub use super::product::{Entity as Product, Model as ProductModel};
pub use product_variant::{Entity as ProductVariant, Model as ProductVariantModel};
pub use cart::{Entity as Cart, Model as CartModel, CartStatus};
pub use cart_item::{Entity as CartItem, Model as CartItemModel};
pub use customer::{Entity as Customer, Model as CustomerModel, CustomerStatus};
pub use customer_group::{Entity as CustomerGroup, Model as CustomerGroupModel};
pub use customer_address::{Entity as CustomerAddress, Model as CustomerAddressModel};
pub use wishlist::{Entity as Wishlist, Model as WishlistModel};
pub use wishlist_item::{Entity as WishlistItem, Model as WishlistItemModel};
pub use category::{Entity as Category, Model as CategoryModel}; 