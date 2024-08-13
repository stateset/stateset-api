pub mod common {
    include!(concat!(env!("OUT_DIR"), "/stateset.common.rs"));
}

pub mod order {
    include!(concat!(env!("OUT_DIR"), "/stateset.order.rs"));
}

pub mod inventory {
    include!(concat!(env!("OUT_DIR"), "/stateset.inventory.rs"));
}

pub mod return_order {
    include!(concat!(env!("OUT_DIR"), "/stateset.return_order.rs"));
}

pub mod warranty {
    include!(concat!(env!("OUT_DIR"), "/stateset.warranty.rs"));
}

pub mod shipment {
    include!(concat!(env!("OUT_DIR"), "/stateset.shipment.rs"));
}

pub mod work_order {
    include!(concat!(env!("OUT_DIR"), "/stateset.work_order.rs"));
}

pub mod billofmaterials {
    include!(concat!(env!("OUT_DIR"), "/stateset.billofmaterials.rs"));
}