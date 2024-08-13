pub mod common {
    tonic::include_proto!("stateset.common");
}

pub mod order {
    tonic::include_proto!("stateset.order");
}

pub mod inventory {
    tonic::include_proto!("stateset.inventory");
}

pub mod customer {
    tonic::include_proto!("stateset.customer");
}