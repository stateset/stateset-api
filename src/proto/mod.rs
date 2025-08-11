pub mod common {
    include!(concat!(env!("OUT_DIR"), "/stateset.common.rs"));
}

pub mod order {
    include!(concat!(env!("OUT_DIR"), "/stateset.order.rs"));
}

pub use order::order_service_server;

pub mod inventory {
    include!(concat!(env!("OUT_DIR"), "/stateset.inventory.rs"));
}

pub use inventory::inventory_service_server;

pub mod return_order {
    include!(concat!(env!("OUT_DIR"), "/stateset.return_order.rs"));
}

pub use return_order::return_service_server;

pub mod warranty {
    include!(concat!(env!("OUT_DIR"), "/stateset.warranty.rs"));
}

pub use warranty::warranty_service_server;

pub mod shipment {
    include!(concat!(env!("OUT_DIR"), "/stateset.shipment.rs"));
}

pub use shipment::shipment_service_server;

pub mod work_order {
    include!(concat!(env!("OUT_DIR"), "/stateset.work_order.rs"));
}

pub use work_order::work_order_service_server;

pub mod billofmaterials {
    include!(concat!(env!("OUT_DIR"), "/stateset.billofmaterials.rs"));
}

pub use billofmaterials::bom_service_server;

// Newly added generated modules
pub mod product {
    include!(concat!(env!("OUT_DIR"), "/stateset.product.rs"));
}

pub use product::product_service_server;

pub mod supplier {
    include!(concat!(env!("OUT_DIR"), "/stateset.supplier.rs"));
}

pub use supplier::supplier_service_server;

pub mod customer {
    include!(concat!(env!("OUT_DIR"), "/stateset.customer.rs"));
}

pub use customer::customer_service_server;

pub mod purchase_order {
    include!(concat!(env!("OUT_DIR"), "/stateset.purchase_order.rs"));
}

pub use purchase_order::purchase_order_service_server;

pub mod asn {
    include!(concat!(env!("OUT_DIR"), "/stateset.asn.rs"));
}

pub use asn::asn_service_server;

pub mod warehouse {
    include!(concat!(env!("OUT_DIR"), "/stateset.warehouse.rs"));
}

pub use warehouse::warehouse_service_server;

pub mod picking {
    include!(concat!(env!("OUT_DIR"), "/stateset.picking.rs"));
}

pub use picking::picking_service_server;

pub mod packaging {
    include!(concat!(env!("OUT_DIR"), "/stateset.packaging.rs"));
}

pub use packaging::packaging_service_server;

pub mod transfer {
    include!(concat!(env!("OUT_DIR"), "/stateset.transfer.rs"));
}

pub use transfer::transfer_service_server;

pub mod payment {
    include!(concat!(env!("OUT_DIR"), "/stateset.payment.rs"));
}

pub use payment::payment_service_server;
