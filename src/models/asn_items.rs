#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "asn_items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub asn_id: Uuid,
    pub purchase_order_item_id: Uuid,
    pub quantity_shipped: i32,
    pub package_number: Option<String>,
    pub lot_number: Option<String>,
    pub serial_numbers: Option<Vec<String>>,
    pub expiration_date: Option<String>,
    pub customs_value: Option<f64>,
    pub country_of_origin: Option<String>,
    pub status: String,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::asn::Entity",
        from = "Column::AsnId",
        to = "super::asn::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    ASN,
    #[sea_orm(
        belongs_to = "super::purchase_order_item::Entity",
        from = "Column::PurchaseOrderItemId",
        to = "super::purchase_order_item::Column::Id",
        on_update = "Cascade",
        on_delete = "Restrict"
    )]
    PurchaseOrderItem,
}