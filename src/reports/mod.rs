use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SalesReport {
    pub total_sales: f64,
    pub total_orders: i32,
    pub average_order_value: f64,
    pub top_selling_products: Vec<(String, i32)>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InventoryReport {
    pub total_products: i32,
    pub total_stock_value: f64,
    pub low_stock_products: Vec<(String, i32)>,
    pub out_of_stock_products: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkOrderEfficiencyReport {
    pub total_work_orders: i32,
    pub completed_work_orders: i32,
    pub average_completion_time: f64,
    pub efficiency_by_user: Vec<(String, f64)>,
}