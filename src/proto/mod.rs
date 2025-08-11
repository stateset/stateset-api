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
